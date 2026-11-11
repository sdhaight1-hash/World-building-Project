//! Scientific source-data loading, source conventions, and editable world annotations.
//! Data files are immutable; annotations use a separate JSON sidecar.
use crate::rainshadow::WindFrom;
use crate::terrain_edit::TerrainEdits;
use crate::terrain_generation::{TerrainMode, TerrainRecipe};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufReader, Cursor, Read, Seek},
    path::{Path, PathBuf},
};
use tiff::decoder::{Decoder, DecodingResult};

#[derive(Debug, Clone)]
pub struct Grid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<f32>,
}

impl Grid {
    pub fn new(width: usize, height: usize, fill: f32) -> Self {
        Self {
            width,
            height,
            cells: vec![fill; width * height],
        }
    }

    pub fn from_tiff(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_reader(BufReader::new(file), path)
    }

    pub fn from_tiff_bytes(bytes: Vec<u8>, path: &Path) -> Result<Self, String> {
        Self::from_reader(Cursor::new(bytes), path)
    }

    fn from_reader<R: Read + Seek>(reader: R, path: &Path) -> Result<Self, String> {
        let mut decoder = Decoder::new(reader).map_err(|e| format!("{}: {e}", path.display()))?;
        let (width, height) = decoder
            .dimensions()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let cells: Vec<f32> = match decoder
            .read_image()
            .map_err(|e| format!("{}: {e}", path.display()))?
        {
            DecodingResult::U8(v) => v.into_iter().map(f32::from).collect(),
            DecodingResult::U16(v) => v.into_iter().map(f32::from).collect(),
            DecodingResult::U32(v) => v.into_iter().map(|n| n as f32).collect(),
            DecodingResult::U64(v) => v.into_iter().map(|n| n as f32).collect(),
            DecodingResult::I8(v) => v.into_iter().map(f32::from).collect(),
            DecodingResult::I16(v) => v.into_iter().map(f32::from).collect(),
            DecodingResult::I32(v) => v.into_iter().map(|n| n as f32).collect(),
            DecodingResult::I64(v) => v.into_iter().map(|n| n as f32).collect(),
            DecodingResult::F32(v) => v,
            DecodingResult::F64(v) => v.into_iter().map(|n| n as f32).collect(),
            other => {
                return Err(format!(
                    "Unsupported TIFF sample type for {}: {other:?}",
                    path.display()
                ))
            }
        };
        if cells.len() != width as usize * height as usize {
            return Err(format!(
                "{}: expected one numeric channel, got {} samples",
                path.display(),
                cells.len()
            ));
        }
        Ok(Self {
            width: width as usize,
            height: height as usize,
            cells,
        })
    }

    #[inline]
    pub fn at(&self, x: usize, y: usize) -> f32 {
        self.cells[y.min(self.height - 1) * self.width + x.min(self.width - 1)]
    }

    pub fn geo(&self, lon_deg: f64, lat_deg: f64) -> f32 {
        let (u, v) = geo_to_uv(lon_deg, lat_deg);
        let x = (u * self.width as f64).floor() as usize % self.width;
        let y = ((v * self.height as f64).floor() as usize).min(self.height - 1);
        self.at(x, y)
    }
}

/// Aegaea's deliberate inverse map convention:
/// +180 at the left, -180 at the right; -90 at the top, +90 at the bottom.
pub fn geo_to_uv(lon_deg: f64, lat_deg: f64) -> (f64, f64) {
    let u = ((180.0 - lon_deg).rem_euclid(360.0)) / 360.0;
    let v = ((lat_deg + 90.0) / 180.0).clamp(0.0, 1.0);
    (u, v)
}

pub fn uv_to_geo(u: f64, v: f64) -> (f64, f64) {
    (180.0 - 360.0 * u, -90.0 + 180.0 * v)
}

#[derive(Debug, Deserialize)]
pub struct PlateInfo {
    pub plate_id: u8,
    pub plate: String,
    pub color_rgb: String,
    #[serde(default)]
    pub euler_pole_lon_deg: Option<f64>,
    #[serde(default)]
    pub euler_pole_lat_deg: Option<f64>,
    #[serde(default, rename = "signed_rate_deg_per_Ma")]
    pub signed_rate_deg_per_ma: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ProvinceInfo {
    pub province_id: u8,
    #[serde(rename = "province")]
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct CratonInfo {
    pub craton_id: u8,
    pub name: String,
    pub plate: String,
    #[serde(rename = "basement_age_Ga")]
    pub basement_age_ga: f32,
}

#[derive(Debug)]
pub struct World {
    pub root: PathBuf,
    pub width: usize,
    pub height: usize,
    pub elevation: Option<Grid>,
    pub slope_deg: Option<Grid>,
    pub plate_ids: Option<Grid>,
    pub craton_ids: Option<Grid>,
    pub province_ids: Option<Grid>,
    pub crust_km: Option<Grid>,
    pub lithosphere_km: Option<Grid>,
    pub heatflow: Option<Grid>,
    pub rain_shadow: Option<Grid>,
    pub plates: Vec<PlateInfo>,
    pub cratons: Vec<CratonInfo>,
    pub provinces: Vec<ProvinceInfo>,
}

impl World {
    pub fn empty(root: PathBuf) -> Self {
        Self {
            root,
            width: 1024,
            height: 512,
            elevation: None,
            slope_deg: None,
            plate_ids: None,
            craton_ids: None,
            province_ids: None,
            crust_km: None,
            lithosphere_km: None,
            heatflow: None,
            rain_shadow: None,
            plates: Vec::new(),
            cratons: Vec::new(),
            provinces: Vec::new(),
        }
    }

    pub fn load(root: PathBuf) -> Result<Self, String> {
        let mut world = Self::empty(root.clone());
        world.elevation = optional_grid(&root, "elevation_m.tif")?;
        world.plate_ids = optional_grid(&root, "plate_id.tif")?;
        world.craton_ids = optional_grid(&root, "craton_id.tif")?;
        world.province_ids = optional_grid(&root, "province_id.tif")?;
        world.crust_km = optional_grid(&root, "crust_km.tif")?;
        world.lithosphere_km = optional_grid(&root, "lithosphere_km.tif")?;
        world.heatflow = optional_grid(&root, "heatflow_mWm2.tif")?;

        let grids = [
            &world.elevation,
            &world.plate_ids,
            &world.craton_ids,
            &world.province_ids,
            &world.crust_km,
            &world.lithosphere_km,
            &world.heatflow,
        ];
        if let Some(first) = grids.iter().find_map(|x| x.as_ref()) {
            (world.width, world.height) = (first.width, first.height);
            for grid in grids.iter().filter_map(|x| x.as_ref()) {
                if grid.width != world.width || grid.height != world.height {
                    return Err(format!(
                        "Raster size mismatch: {}x{} vs {}x{} in {}",
                        grid.width,
                        grid.height,
                        world.width,
                        world.height,
                        root.display()
                    ));
                }
            }
        }
        let plates_path = root.join("plates.csv");
        if plates_path.exists() {
            let mut reader = csv::Reader::from_path(&plates_path).map_err(|e| e.to_string())?;
            for plate in reader.deserialize() {
                world
                    .plates
                    .push(plate.map_err(|e| format!("{}: {e}", plates_path.display()))?);
            }
        }
        let cratons_path = root.join("cratons.csv");
        if cratons_path.exists() {
            let mut reader = csv::Reader::from_path(&cratons_path).map_err(|e| e.to_string())?;
            for craton in reader.deserialize() {
                world
                    .cratons
                    .push(craton.map_err(|e| format!("{}: {e}", cratons_path.display()))?);
            }
        }
        let provinces_path = root.join("crustal_provinces.csv");
        if provinces_path.exists() {
            let mut reader = csv::Reader::from_path(&provinces_path).map_err(|e| e.to_string())?;
            for row in reader.deserialize() {
                world
                    .provinces
                    .push(row.map_err(|e| format!("{}: {e}", provinces_path.display()))?);
            }
        }
        Ok(world)
    }

    pub fn has_any_source(&self) -> bool {
        self.elevation.is_some() || self.plate_ids.is_some() || self.craton_ids.is_some()
    }

    pub fn plate_at(&self, lon: f64, lat: f64) -> Option<&PlateInfo> {
        let id = self.plate_ids.as_ref()?.geo(lon, lat) as u8;
        self.plates.iter().find(|p| p.plate_id == id)
    }

    pub fn craton_at(&self, lon: f64, lat: f64) -> Option<&CratonInfo> {
        let id = self.craton_ids.as_ref()?.geo(lon, lat) as u8;
        self.cratons.iter().find(|c| c.craton_id == id)
    }

    pub fn elevation_at(&self, lon: f64, lat: f64) -> Option<f32> {
        self.elevation.as_ref().map(|g| g.geo(lon, lat))
    }

    pub fn rain_shadow_at(&self, lon: f64, lat: f64) -> Option<f32> {
        self.rain_shadow.as_ref().map(|g| g.geo(lon, lat))
    }
}

fn optional_grid(root: &Path, name: &str) -> Result<Option<Grid>, String> {
    let path = root.join(name);
    if path.exists() {
        return Grid::from_tiff(&path).map(Some);
    }
    // The reference elevation source is committed in byte-for-byte Git parts.
    // Read it without modifying the immutable dataset or requiring Python.
    let mut bytes = Vec::new();
    for index in 0..10_000usize {
        let part = root.join(format!(".binary-parts/{name}.part{index:03}"));
        if !part.exists() {
            break;
        }
        bytes.extend_from_slice(&fs::read(&part).map_err(|e| format!("{}: {e}", part.display()))?);
    }
    if bytes.is_empty() {
        Ok(None)
    } else {
        Grid::from_tiff_bytes(bytes, &path).map(Some)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Marker {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub longitude_deg: f64,
    pub latitude_deg: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default)]
    pub wind_from: WindFrom,
    #[serde(default)]
    pub terrain: TerrainEdits,
    #[serde(default)]
    pub terrain_mode: TerrainMode,
    #[serde(default)]
    pub terrain_recipe: TerrainRecipe,
}

impl Project {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            markers: Vec::new(),
            wind_from: WindFrom::default(),
            terrain: TerrainEdits::default(),
            terrain_mode: TerrainMode::default(),
            terrain_recipe: TerrainRecipe::default(),
        }
    }

    pub fn load(path: &Path, default_name: &str) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::new(default_name));
        }
        let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or_else(|| "Project path has no parent".to_owned())?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let content = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        // Make a backup before replacing an existing project. P0 is not an
        // atomic transactional database; this is only a first safety measure.
        if path.exists() {
            fs::copy(path, path.with_extension("json.bak")).map_err(|e| e.to_string())?;
        }
        fs::write(path, content).map_err(|e| e.to_string())
    }

    pub fn add_marker(&mut self, lon: f64, lat: f64) -> usize {
        let id = self.markers.iter().map(|m| m.id).max().unwrap_or(0) + 1;
        self.markers.push(Marker {
            id,
            name: format!("Location {id}"),
            description: String::new(),
            longitude_deg: lon,
            latitude_deg: lat,
        });
        self.markers.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirrored_inverse_coordinates_roundtrip() {
        for (lon, lat) in [(0.0, 0.0), (-124.0, 45.0), (87.0, -34.0), (179.0, 90.0)] {
            let (u, v) = geo_to_uv(lon, lat);
            let (out_lon, out_lat) = uv_to_geo(u, v);
            assert!((lon - out_lon).abs() < 1e-8);
            assert!((lat - out_lat).abs() < 1e-8);
        }
        assert_eq!(uv_to_geo(0.0, 0.0), (180.0, -90.0));
        assert_eq!(uv_to_geo(1.0, 1.0), (-180.0, 90.0));
    }

    #[test]
    fn geographic_sampling_wraps_longitude() {
        let grid = Grid {
            width: 4,
            height: 2,
            cells: vec![0., 1., 2., 3., 4., 5., 6., 7.],
        };
        assert_eq!(grid.geo(179.9, -89.0), 0.0);
        assert_eq!(grid.geo(-179.9, -89.0), 3.0);
        assert_eq!(grid.geo(180.1, -89.0), 3.0);
        assert_eq!(grid.geo(0.0, 89.0), 6.0);
    }

    #[test]
    fn blank_project_has_no_reference_world() {
        let blank = World::empty(PathBuf::from("other-planet"));
        assert!(blank.plates.is_empty());
        assert!(blank.cratons.is_empty());
        assert!(blank.provinces.is_empty());
        assert!(blank.slope_deg.is_none());
        assert!(!blank.has_any_source());
    }

    #[test]
    fn aegreea_split_elevation_is_loadable_directly() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/aegaea/tectonics/v1");
        let world = World::load(root).expect("committed Aegaea rasters should load");
        assert_eq!((world.width, world.height), (1672, 941));
        let elevation = world.elevation.expect("split reference DEM should load");
        assert!(elevation.cells.iter().any(|&v| v > 0.0));
        assert!(elevation.cells.iter().any(|&v| v < 0.0));
    }

    #[test]
    fn markers_roundtrip_without_sources() {
        let mut p = Project::new("Test World");
        p.add_marker(-122.0, 48.0);
        let json = serde_json::to_string(&p).unwrap();
        let q: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(q.markers[0].longitude_deg, -122.0);
        assert_eq!(q.name, "Test World");
        assert_eq!(q.wind_from, WindFrom::West);
        assert_eq!(q.terrain_mode, TerrainMode::Auto);
        assert_eq!(q.terrain_recipe, TerrainRecipe::default());
    }
}
