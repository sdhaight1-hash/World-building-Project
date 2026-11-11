//! Typed, deterministic raster-processing graphs for natural-world maps.
//!
//! This first graph engine has NO hidden simulation and does NOT edit source
//! geological fields. Each node transforms a named, unit-checked raster, and
//! each output retains its recipe. This is the reusable foundation on which
//! terrain, climate, and geology tools can grow.
use crate::{
    rainshadow::{self, WindFrom},
    world::{Grid, World},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    f64::consts::PI,
    fs::File,
    path::Path,
    sync::Arc,
};
use tiff::encoder::{colortype, TiffEncoder};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quantity {
    ElevationMetres,
    SlopeDegrees,
    RainShadowIndex,
}

impl Quantity {
    pub fn units(self) -> &'static str {
        match self {
            Self::ElevationMetres => "m",
            Self::SlopeDegrees => "degrees",
            Self::RainShadowIndex => "signed dimensionless [-1, 1]",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Field {
    pub quantity: Quantity,
    pub grid: Arc<Grid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operator", rename_all = "snake_case")]
pub enum Operator {
    /// Read the existing DEM, including manually authored project overrides.
    Elevation,
    /// Change an elevation field; never silently changes imported source data.
    ElevationOffset { input: String, delta_m: f32 },
    /// Compute the physical surface slope in degrees from a spherical DEM.
    Slope {
        input: String,
        planet_radius_km: f64,
    },
    /// The existing directional *terrain-only* rain shadow approximation.
    OrographicExposure {
        input: String,
        wind_from: WindFrom,
    },
    /// Explicit named output. Future nodes will use typed, named ports.
    Output { input: String },
}

impl Operator {
    fn input(&self) -> Option<&str> {
        match self {
            Self::Elevation => None,
            Self::ElevationOffset { input, .. }
            | Self::Slope { input, .. }
            | Self::OrographicExposure { input, .. }
            | Self::Output { input } => Some(input),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(flatten)]
    pub operator: Operator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapGraph {
    pub version: u32,
    pub name: String,
    pub nodes: Vec<Node>,
    pub outputs: Vec<String>,
}

#[derive(Debug)]
pub struct Build {
    pub outputs: BTreeMap<String, Field>,
    pub evaluated_nodes: usize,
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

struct Evaluator<'a> {
    world: &'a World,
    definitions: HashMap<&'a str, &'a Operator>,
    visiting: HashSet<String>,
    cache: HashMap<String, Field>,
}

impl<'a> Evaluator<'a> {
    fn resolve(&mut self, name: &str) -> Result<Field, String> {
        if let Some(value) = self.cache.get(name) {
            return Ok(value.clone());
        }
        if !self.visiting.insert(name.to_owned()) {
            return Err(format!("Cycle in map graph at node '{name}'"));
        }
        let operator = self
            .definitions
            .get(name)
            .copied()
            .ok_or_else(|| format!("Map graph references missing node '{name}'"))?;
        let source = if let Some(input) = operator.input() {
            Some(self.resolve(input)?)
        } else {
            None
        };
        let result = match operator {
            Operator::Elevation => {
                let grid = self.world.elevation.as_ref().ok_or_else(|| {
                    "Elevation node requires an imported or authored DEM".to_owned()
                })?;
                Field {
                    quantity: Quantity::ElevationMetres,
                    grid: Arc::new(grid.clone()),
                }
            }
            Operator::ElevationOffset { delta_m, .. } => {
                let source = expect_unit(source, Quantity::ElevationMetres, name)?;
                if !delta_m.is_finite() {
                    return Err(format!("Non-finite elevation offset at '{name}'"));
                }
                let mut grid = (*source.grid).clone();
                for value in &mut grid.cells {
                    if value.is_finite() {
                        *value += delta_m;
                        if !value.is_finite() {
                            return Err(format!("Elevation overflow at '{name}'"));
                        }
                    }
                }
                Field {
                    grid: Arc::new(grid),
                    quantity: Quantity::ElevationMetres,
                }
            }
            Operator::Slope {
                planet_radius_km, ..
            } => {
                let source = expect_unit(source, Quantity::ElevationMetres, name)?;
                Field {
                    quantity: Quantity::SlopeDegrees,
                    grid: Arc::new(slope_degrees(&source.grid, *planet_radius_km)?),
                }
            }
            Operator::OrographicExposure { wind_from, .. } => {
                let source = expect_unit(source, Quantity::ElevationMetres, name)?;
                Field {
                    quantity: Quantity::RainShadowIndex,
                    grid: Arc::new(rainshadow::derive(&source.grid, *wind_from)),
                }
            }
            Operator::Output { .. } => source
                .ok_or_else(|| format!("Output '{name}' has no input"))?,
        };
        self.visiting.remove(name);
        self.cache.insert(name.to_owned(), result.clone());
        Ok(result)
    }
}

fn expect_unit(
    source: Option<Field>,
    expected: Quantity,
    name: &str,
) -> Result<Field, String> {
    let source = source.ok_or_else(|| format!("Node '{name}' has no input"))?;
    if source.quantity != expected {
        return Err(format!(
            "Node '{name}' needs {} input, received {}",
            expected.units(),
            source.quantity.units()
        ));
    }
    Ok(source)
}

impl MapGraph {
    pub fn from_json(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn build(&self, world: &World) -> Result<Build, String> {
        if self.version != 1 {
            return Err(format!(
                "Unsupported map graph version {}; expected 1",
                self.version
            ));
        }
        if self.nodes.is_empty() || self.outputs.is_empty() {
            return Err("Map graph needs nodes and at least one output".into());
        }
        let mut definitions = HashMap::with_capacity(self.nodes.len());
        for node in &self.nodes {
            if !safe_id(&node.id) {
                return Err(format!("Invalid graph node ID '{}'", node.id));
            }
            if definitions.insert(node.id.as_str(), &node.operator).is_some() {
                return Err(format!("Duplicate graph node ID '{}'", node.id));
            }
        }
        let mut evaluator = Evaluator {
            world,
            definitions,
            visiting: HashSet::new(),
            cache: HashMap::new(),
        };
        // Validate the full graph, including nodes not currently published.
        for node in &self.nodes {
            evaluator.resolve(&node.id)?;
        }
        let mut results = BTreeMap::new();
        for id in &self.outputs {
            if !safe_id(id) {
                return Err(format!("Invalid output ID '{id}'"));
            }
            if !matches!(evaluator.definitions.get(id.as_str()), Some(Operator::Output { .. })) {
                return Err(format!("Published output '{id}' must be an Output node"));
            }
            let field = evaluator.resolve(id)?;
            if results.insert(id.clone(), field).is_some() {
                return Err(format!("Duplicate output '{id}'"));
            }
        }
        Ok(Build {
            outputs: results,
            evaluated_nodes: evaluator.cache.len(),
        })
    }
}

/// Surface gradient, physical spherical distances, and **Aegaea's inverted atlas orientation**.
/// Slope is orientation-independent but not pixel-size-independent, especially near poles.
pub fn slope_degrees(elevation: &Grid, radius_km: f64) -> Result<Grid, String> {
    if !radius_km.is_finite() || radius_km <= 0.0 {
        return Err("Planet radius must be finite and positive".into());
    }
    if elevation.width < 3 || elevation.height < 3 {
        return Err("Slope requires an elevation grid at least 3x3".into());
    }
    let mut slope = Grid::new(elevation.width, elevation.height, f32::NAN);
    let radius_m = radius_km * 1000.0;
    let y_spacing = PI * radius_m / elevation.height as f64;
    for y in 0..elevation.height {
        let latitude = -PI / 2.0 + PI * (y as f64 + 0.5) / elevation.height as f64;
        // Guard the coordinate singularity without pretending polar slope is precise.
        let x_spacing = (2.0 * PI * radius_m * latitude.cos().abs()
            / elevation.width as f64).max(1.0);
        let north = (y + 1).min(elevation.height - 1);
        let south = y.saturating_sub(1);
        let y_denominator = (north - south) as f64 * y_spacing;
        for x in 0..elevation.width {
            let east = (x + elevation.width - 1) % elevation.width;
            let west = (x + 1) % elevation.width;
            let samples = [
                elevation.at(x,y), elevation.at(east,y), elevation.at(west,y),
                elevation.at(x,north), elevation.at(x,south),
            ];
            if !samples.iter().all(|v|v.is_finite()) {
                continue;
            }
            let dx = (samples[1] as f64 - samples[2] as f64) / (2.0 * x_spacing);
            let dy = (samples[3] as f64 - samples[4] as f64) / y_denominator;
            slope.cells[y * elevation.width + x] =
                dx.hypot(dy).atan().to_degrees() as f32;
        }
    }
    Ok(slope)
}

/// Export a numeric graph output and a JSON sidecar with its scientific units,
/// recipe and source provenance; this is a TIFF, **not an Earth GeoTIFF**.
pub fn export_outputs(
    graph: &MapGraph,
    build: &Build,
    dir: &Path,
) -> Result<usize, String> {
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    for (id,field) in &build.outputs {
        let path = dir.join(format!("{id}.tif"));
        let file=File::create(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let mut encoder=TiffEncoder::new(file)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        encoder.write_image::<colortype::Gray32Float>(
            field.grid.width as u32,
            field.grid.height as u32,
            &field.grid.cells,
        ).map_err(|e| format!("{}: {e}", path.display()))?;
        let metadata=serde_json::json!({
            "graph_name":graph.name,
            "graph_version":graph.version,
            "output_node":id,
            "quantity":field.quantity,
            "units":field.quantity.units(),
            "grid":[field.grid.width,field.grid.height],
            "coordinate_convention":{
                "longitude":"180 - 360*(x+0.5)/width",
                "latitude":"-90 + 180*(y+0.5)/height",
                "not_earth_epsg_4326":true
            },
            "model_status":"deterministic authored/procedural field; not a validated geophysical simulation",
            "recipe":graph
        });
        std::fs::write(
            dir.join(format!("{id}.json")),
            serde_json::to_vec_pretty(&metadata).map_err(|e|e.to_string())?
        ).map_err(|e|e.to_string())?;
    }
    Ok(build.outputs.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn world() -> World {
        let mut world=World::empty(PathBuf::from("unrelated-world"));
        world.width=12;
        world.height=6;
        let mut grid=Grid::new(12,6,50.0);
        for y in 0..6 {
            for x in 0..12 {grid.cells[y*12+x] += x as f32 * 30.0;}
        }
        world.elevation=Some(grid);
        world
    }

    fn sample_graph() -> MapGraph {
        MapGraph {
            version:1,
            name:"Example natural fields".into(),
            nodes:vec![
                Node{id:"dem".into(),operator:Operator::Elevation},
                Node{id:"ramped".into(),operator:Operator::ElevationOffset {
                    input:"dem".into(),delta_m:125.0
                }},
                Node{id:"slope".into(),operator:Operator::Slope {
                    input:"ramped".into(),planet_radius_km:6371.0
                }},
                Node{id:"map".into(),operator:Operator::Output{input:"slope".into()}},
            ],
            outputs:vec!["map".into()],
        }
    }

    #[test]
    fn graph_is_deterministic_and_source_is_immutable() {
        let world=world();
        let source=world.elevation.as_ref().unwrap().cells.clone();
        let graph=sample_graph();
        let a=graph.build(&world).unwrap();
        let b=graph.build(&world).unwrap();
        assert_eq!(a.evaluated_nodes,4);
        assert_eq!(a.outputs["map"].quantity,Quantity::SlopeDegrees);
        assert_eq!(a.outputs["map"].grid.cells,b.outputs["map"].grid.cells);
        assert_eq!(source,world.elevation.unwrap().cells);
    }

    #[test]
    fn shared_dependencies_are_evaluated_only_once() {
        let mut graph=sample_graph();
        graph.nodes.push(Node {
            id:"other".into(),operator:Operator::Output{input:"slope".into()}
        });
        graph.outputs.push("other".into());
        let build=graph.build(&world()).unwrap();
        assert_eq!(build.evaluated_nodes,5);
        assert!(Arc::ptr_eq(&build.outputs["map"].grid,&build.outputs["other"].grid));
    }

    #[test]
    fn cycles_and_missing_references_fail_with_explanations() {
        let mut graph=sample_graph();
        graph.nodes[1].operator=Operator::ElevationOffset {
            input:"slope".into(),delta_m:1.0
        };
        assert!(graph.build(&world()).unwrap_err().contains("Cycle"));
        graph.nodes[1].operator=Operator::ElevationOffset {
            input:"nonexistent".into(),delta_m:1.0
        };
        assert!(graph.build(&world()).unwrap_err().contains("missing"));
    }

    #[test]
    fn units_block_physically_meaningless_connections() {
        let mut graph=sample_graph();
        graph.nodes.push(Node {
            id:"nonsense".into(),operator:Operator::ElevationOffset {
                input:"slope".into(),delta_m:1.0
            }
        });
        assert!(graph.build(&world()).unwrap_err().contains("needs m input"));
    }

    #[test]
    fn invalid_output_names_cannot_traverse_directories() {
        let mut graph=sample_graph();
        graph.nodes[3].id="../outside".into();
        graph.outputs=vec!["../outside".into()];
        assert!(graph.build(&world()).unwrap_err().contains("Invalid graph node ID"));
    }

    #[test]
    fn flat_world_has_zero_slope_and_ridges_have_positive_slope() {
        let flat=Grid::new(32,16,500.0);
        let out=slope_degrees(&flat,6371.0).unwrap();
        assert!(out.cells.iter().all(|v|v.abs()<1e-6));
        let mut peak=flat;
        peak.cells[8*32+14]=2000.0;
        let out=slope_degrees(&peak,6371.0).unwrap();
        assert!(out.at(15,8)>0.0);
        assert!(slope_degrees(&out,0.0).is_err());
    }

    #[test]
    fn numeric_tiff_and_recipe_metadata_round_trip() {
        let graph=sample_graph();
        let build=graph.build(&world()).unwrap();
        let dir=std::env::temp_dir().join(format!(
            "aegaea-node-graph-test-{}",std::process::id()));
        assert_eq!(export_outputs(&graph,&build,&dir).unwrap(),1);
        let grid=Grid::from_tiff(&dir.join("map.tif")).unwrap();
        assert_eq!(grid.cells,build.outputs["map"].grid.cells);
        let metadata=std::fs::read_to_string(dir.join("map.json")).unwrap();
        assert!(metadata.contains("slope_degrees"));
        assert!(metadata.contains("ramped"));
        let _=std::fs::remove_dir_all(dir);
    }

    #[test]
    fn real_aegaea_dem_works_with_the_same_graph() {
        let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../data/aegaea/tectonics/v1");
        let world=World::load(root).unwrap();
        let result=sample_graph().build(&world).unwrap();
        let grid=&result.outputs["map"].grid;
        assert_eq!((grid.width,grid.height),(1672,941));
        assert!(grid.cells.iter().any(|v|v.is_finite() && *v>0.0));
    }
}
