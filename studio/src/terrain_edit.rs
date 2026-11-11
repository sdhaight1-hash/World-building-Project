//! Non-destructive raster terrain painting for physical-world maps.
//!
//! Sparse project-local absolute-elevation overrides preserve the immutable
//! imported DEM. Author edits are separate from simulations or measurements.
//! Brush strokes are transactional undo/redo operations in RAM.
use crate::world::{geo_to_uv, Grid, World};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs::File, path::Path};
use tiff::encoder::{colortype, TiffEncoder};

const MIN_ELEVATION_M: f32 = -12_000.0;
const MAX_ELEVATION_M: f32 = 12_000.0;

/// The project sidecar, not the source GeoTIFF, owns these values.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TerrainEdits {
    /// A project created without an imported DEM may start from a constant
    /// elevation, e.g. a wholly submerged world at -2000 metres.
    #[serde(default)]
    pub base_fill_m: Option<f32>,
    /// Prevent accidental application to a different-sized source raster.
    #[serde(default)]
    pub grid_size: Option<[usize; 2]>,
    /// Linear pixel index -> absolute authored elevation (metres).
    #[serde(default)]
    pub overrides: BTreeMap<usize, f32>,
}

impl TerrainEdits {
    pub fn apply(&self, world: &mut World) -> Result<(), String> {
        if let Some(fill) = self.base_fill_m {
            if !fill.is_finite() || !(MIN_ELEVATION_M..=MAX_ELEVATION_M).contains(&fill) {
                return Err("Invalid authored base elevation".into());
            }
            if world.elevation.is_none() {
                world.elevation = Some(Grid::new(world.width, world.height, fill));
            }
        }
        if self.overrides.is_empty() {
            return Ok(());
        }
        let grid = world
            .elevation
            .as_mut()
            .ok_or("Terrain overrides require a source or created elevation grid")?;
        if let Some([width, height]) = self.grid_size {
            if (grid.width, grid.height) != (width, height) {
                return Err(format!(
                    "Terrain edit grid {width}x{height} differs from source {}x{}",
                    grid.width, grid.height
                ));
            }
        }
        for (&index, &elevation) in &self.overrides {
            if index >= grid.cells.len()
                || !elevation.is_finite()
                || !(MIN_ELEVATION_M..=MAX_ELEVATION_M).contains(&elevation)
            {
                return Err(format!("Invalid terrain override at cell {index}"));
            }
        }
        for (&index, &elevation) in &self.overrides {
            grid.cells[index] = elevation;
        }
        Ok(())
    }

    pub fn create_flat_world(&mut self, world: &mut World, fill_m: f32) -> Result<(), String> {
        if world.elevation.is_some() || self.base_fill_m.is_some() {
            return Err("World already has an elevation layer".into());
        }
        if !fill_m.is_finite() || !(MIN_ELEVATION_M..=MAX_ELEVATION_M).contains(&fill_m) {
            return Err("Invalid elevation".into());
        }
        self.base_fill_m = Some(fill_m);
        self.grid_size = Some([world.width, world.height]);
        world.elevation = Some(Grid::new(world.width, world.height, fill_m));
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct CellChange {
    pub before: f32,
    pub after: f32,
    pub previous_override: Option<f32>,
}

#[derive(Default, Clone, Debug)]
pub struct Stroke {
    /// At most one final change per raster cell, even for repeated drag dabs.
    pub changes: BTreeMap<usize, CellChange>,
}

impl Stroke {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Combine successive paint dabs into a single reversible user gesture.
    pub fn merge(&mut self, other: Stroke) {
        for (index, edit) in other.changes {
            self.changes
                .entry(index)
                .and_modify(|old| old.after = edit.after)
                .or_insert(edit);
        }
    }

    pub fn undo(&self, world: &mut World, edits: &mut TerrainEdits) {
        let Some(grid) = world.elevation.as_mut() else {
            return;
        };
        for (&index, edit) in &self.changes {
            grid.cells[index] = edit.before;
            match edit.previous_override {
                Some(value) => {
                    edits.overrides.insert(index, value);
                }
                None => {
                    edits.overrides.remove(&index);
                }
            }
        }
    }

    pub fn redo(&self, world: &mut World, edits: &mut TerrainEdits) {
        let Some(grid) = world.elevation.as_mut() else {
            return;
        };
        for (&index, edit) in &self.changes {
            grid.cells[index] = edit.after;
            edits.overrides.insert(index, edit.after);
        }
    }
}

/// Apply a terrain elevation delta in metres with quadratic radial falloff.
/// The brush wraps geographic longitude at the seam and clamps latitude.
/// It deliberately changes no geological plate/craton data.
pub fn brush_dab(
    world: &mut World,
    edits: &mut TerrainEdits,
    longitude: f64,
    latitude: f64,
    radius_cells: u32,
    delta_m: f32,
) -> Result<Stroke, String> {
    let grid = world
        .elevation
        .as_mut()
        .ok_or("No DEM for terrain painting")?;
    let radius = radius_cells.clamp(1, 40) as isize;
    let (u, v) = geo_to_uv(longitude, latitude);
    let cx = (u * grid.width as f64).floor() as isize;
    let cy = ((v * grid.height as f64).floor() as isize).min(grid.height as isize - 1);
    let mut stroke = Stroke::default();
    if !delta_m.is_finite() {
        return Err("Brush delta must be finite".into());
    }
    edits.grid_size = Some([grid.width, grid.height]);
    for oy in -radius..=radius {
        let y = cy + oy;
        if y < 0 || y >= grid.height as isize {
            continue;
        }
        for ox in -radius..=radius {
            let distance = ((ox * ox + oy * oy) as f64).sqrt();
            if distance > radius as f64 {
                continue;
            }
            let x = (cx + ox).rem_euclid(grid.width as isize) as usize;
            let index = y as usize * grid.width + x;
            let before = grid.cells[index];
            if !before.is_finite() {
                continue;
            }
            let influence = 1.0 - (distance / (radius as f64));
            // Minimum 1% effect at the edge of the brush center region.
            let amount = (delta_m as f64 * influence.powi(2)) as f32;
            if amount.abs() < 0.005 {
                continue;
            }
            let after = (before + amount).clamp(MIN_ELEVATION_M, MAX_ELEVATION_M);
            if before == after {
                continue;
            }
            stroke.changes.insert(
                index,
                CellChange {
                    before,
                    after,
                    previous_override: edits.overrides.get(&index).copied(),
                },
            );
            grid.cells[index] = after;
            edits.overrides.insert(index, after);
        }
    }
    Ok(stroke)
}

/// Numeric full-resolution export of the current authored elevation grid.
pub fn export_tiff(world: &World, edits: &TerrainEdits, path: &Path) -> Result<(), String> {
    let grid = world
        .elevation
        .as_ref()
        .ok_or("No elevation raster to export")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = File::create(path).map_err(|e| e.to_string())?;
    let mut tiff = TiffEncoder::new(file).map_err(|e| e.to_string())?;
    tiff.write_image::<colortype::Gray32Float>(grid.width as u32, grid.height as u32, &grid.cells)
        .map_err(|e| e.to_string())?;
    let metadata = serde_json::json!({
        "layer":"elevation_m",
        "units":"metres relative to fictional planet sea-level datum",
        "width":grid.width,
        "height":grid.height,
        "authored_pixel_count":edits.overrides.len(),
        "base_fill_m":edits.base_fill_m,
        "longitude_pixel_center":"180 - 360*(x+0.5)/width",
        "latitude_pixel_center":"-90 + 180*(y+0.5)/height",
        "source":"immutable imported DEM plus project-local authored overrides",
        "status":"authored elevation, not a scientifically validated terrain reconstruction",
        "earth_epsg_4326":false
    });
    std::fs::write(
        path.with_extension("json"),
        serde_json::to_vec_pretty(&metadata).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn world() -> World {
        let mut w = World::empty(PathBuf::from("custom-world"));
        w.width = 16;
        w.height = 8;
        w.elevation = Some(Grid::new(16, 8, 100.0));
        w
    }

    #[test]
    fn painting_and_undo_redo_keep_source_mutation_reversible() {
        let mut w = world();
        let mut project = TerrainEdits::default();
        let dab = brush_dab(&mut w, &mut project, 0.0, 0.0, 2, 250.0).unwrap();
        assert!(!dab.is_empty());
        assert!(w
            .elevation
            .as_ref()
            .unwrap()
            .cells
            .iter()
            .any(|&v| v > 100.0));
        let edited = project.overrides.clone();
        dab.undo(&mut w, &mut project);
        assert!(project.overrides.is_empty());
        assert!(w
            .elevation
            .as_ref()
            .unwrap()
            .cells
            .iter()
            .all(|&v| v == 100.0));
        dab.redo(&mut w, &mut project);
        assert_eq!(edited, project.overrides);
    }

    #[test]
    fn repeated_drag_dabs_are_one_undo_action() {
        let mut w = world();
        let mut edits = TerrainEdits::default();
        let a = brush_dab(&mut w, &mut edits, 0.0, 0.0, 2, 100.0).unwrap();
        let b = brush_dab(&mut w, &mut edits, 0.0, 0.0, 2, 100.0).unwrap();
        let mut grouped = a;
        grouped.merge(b);
        grouped.undo(&mut w, &mut edits);
        assert!(w
            .elevation
            .as_ref()
            .unwrap()
            .cells
            .iter()
            .all(|&v| v == 100.0));
        assert!(edits.overrides.is_empty());
    }

    #[test]
    fn seam_painting_wraps_geographic_longitude() {
        let mut w = world();
        let mut edits = TerrainEdits::default();
        brush_dab(&mut w, &mut edits, 179.9, 0.0, 2, 150.0).unwrap();
        let row = 4usize;
        let grid = w.elevation.unwrap();
        assert!(grid.at(0, row) > 100.0);
        assert!(grid.at(15, row) > 100.0);
    }

    #[test]
    fn sparse_project_edits_reload_onto_fresh_dem() {
        let mut w = world();
        let mut edits = TerrainEdits::default();
        brush_dab(&mut w, &mut edits, -50.0, 0.0, 2, 99.0).unwrap();
        let expected = w.elevation.unwrap().cells;
        let encoded = serde_json::to_string(&edits).unwrap();
        let restored: TerrainEdits = serde_json::from_str(&encoded).unwrap();
        let mut fresh = world();
        restored.apply(&mut fresh).unwrap();
        assert_eq!(expected, fresh.elevation.unwrap().cells);
        let mut mismatch = world();
        mismatch.width = 8;
        mismatch.elevation = Some(Grid::new(8, 8, 100.0));
        assert!(restored.apply(&mut mismatch).is_err());
    }

    #[test]
    fn blank_world_can_create_its_own_dem() {
        let mut w = World::empty(PathBuf::from("blank"));
        let mut edits = TerrainEdits::default();
        edits.create_flat_world(&mut w, -2000.0).unwrap();
        assert_eq!(w.elevation.as_ref().unwrap().cells[0], -2000.0);
        let mut restored = World::empty(PathBuf::from("blank"));
        edits.apply(&mut restored).unwrap();
        assert_eq!(restored.elevation.unwrap().cells[0], -2000.0);
    }

    #[test]
    fn tiff_export_roundtrip_is_numeric_not_just_png() {
        let mut w = world();
        let mut edits = TerrainEdits::default();
        brush_dab(&mut w, &mut edits, 0.0, 0.0, 2, 333.0).unwrap();
        let path =
            std::env::temp_dir().join(format!("aegaea-terrain-test-{}.tif", std::process::id()));
        export_tiff(&w, &edits, &path).unwrap();
        let reread = Grid::from_tiff(&path).unwrap();
        assert_eq!(reread.cells, w.elevation.unwrap().cells);
        let _ = std::fs::remove_file(path.with_extension("json"));
        let _ = std::fs::remove_file(path);
    }
}
