//! Directional terrain-only orographic exposure / rain-shadow *potential*.
//!
//! Inputs: a real native-resolution elevation grid and a user-selected wind
//! direction. Output: signed, dimensionless index in [-1, 1]:
//!   negative = windward orographic uplift potential
//!   positive = leeward shelter/rain-shadow potential
//!   NaN = ocean or missing elevation.
//!
//! This is intentionally NOT precipitation, humidity, a circulation model,
//! or an inferred climate classification. The reference model does not yet
//! have seasonally resolved winds or water-vapor transport. See docs/RAIN_SHADOW.md.

use crate::world::Grid;
use serde::{Deserialize, Serialize};
use std::{f64::consts::PI, fs::File, path::Path};
use tiff::encoder::{colortype, TiffEncoder};

const PLANET_RADIUS_KM: f64 = 6371.0; // Aegaea starter value; parameterized planet radii come later.
const TRACE_STEP_KM: f64 = 35.0;
const MAX_UPWIND_KM: f64 = 700.0;
const LOCAL_UPLIFT_KM: f64 = 105.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WindFrom {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    #[default]
    West,
    NorthWest,
}

impl WindFrom {
    pub const ALL: [Self; 8] = [
        Self::North,
        Self::NorthEast,
        Self::East,
        Self::SouthEast,
        Self::South,
        Self::SouthWest,
        Self::West,
        Self::NorthWest,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::North => "North",
            Self::NorthEast => "Northeast",
            Self::East => "East",
            Self::SouthEast => "Southeast",
            Self::South => "South",
            Self::SouthWest => "Southwest",
            Self::West => "West",
            Self::NorthWest => "Northwest",
        }
    }

    /// Compass bearing (clockwise from geographic north) towards which wind blows.
    pub fn to_bearing_deg(self) -> f64 {
        let from: f64 = match self {
            Self::North => 0.0,
            Self::NorthEast => 45.0,
            Self::East => 90.0,
            Self::SouthEast => 135.0,
            Self::South => 180.0,
            Self::SouthWest => 225.0,
            Self::West => 270.0,
            Self::NorthWest => 315.0,
        };
        (from + 180.0) % 360.0
    }
}

/// Bilinear sample on a periodically wrapped east-west grid, with limited poles.
/// This is exclusively a numerical helper. It does not rotate the source raster.
fn interpolated_height(grid: &Grid, x: f64, y: f64) -> f32 {
    let width = grid.width as isize;
    let x0 = x.floor() as isize;
    let y0 = (y.floor() as usize).min(grid.height - 1);
    let x1 = (x0 + 1).rem_euclid(width) as usize;
    let x0 = x0.rem_euclid(width) as usize;
    let y1 = (y0 + 1).min(grid.height - 1);
    let fx = (x - x.floor()) as f32;
    let fy = (y - y.floor()) as f32;
    let a = grid.at(x0, y0) * (1.0 - fx) + grid.at(x1, y0) * fx;
    let b = grid.at(x0, y1) * (1.0 - fx) + grid.at(x1, y1) * fx;
    a * (1.0 - fy) + b * fy
}

/// Estimate windward/lee orographic *exposure* without simulating precipitation.
///
/// Upwind peaks create a leeward signal which decays with horizontal distance.
/// Rising ground relative to the nearby upwind profile creates a windward signal.
/// The scan stops at the first ocean crossing so an intervening moisture source
/// prevents distant mountains from incorrectly shadowing land behind that ocean.
///
/// Wind is assumed horizontally uniform at all latitudes in this first version.
/// Polar pixels are approximate, since longitude spacing tends toward zero there.
pub fn derive(elevation: &Grid, wind_from: WindFrom) -> Grid {
    let width = elevation.width;
    let height = elevation.height;
    let mut result = Grid::new(width, height, f32::NAN);
    let to = wind_from.to_bearing_deg().to_radians();
    let (north, east) = (to.cos(), to.sin());
    let lat_cell_km = PI * PLANET_RADIUS_KM / height as f64;
    let samples = (MAX_UPWIND_KM / TRACE_STEP_KM) as usize;
    for y in 0..height {
        let latitude = -90.0 + 180.0 * (y as f64 + 0.5) / height as f64;
        let lon_cell_km = (2.0 * PI * PLANET_RADIUS_KM * latitude.to_radians().cos().abs()
            / width as f64)
            .max(0.5);
        // In this fictional atlas, increasing x means WEST and increasing y means NORTH.
        let dx_per_km = east / lon_cell_km;
        let dy_per_km = -north / lat_cell_km;
        for x in 0..width {
            let base = elevation.at(x, y);
            if !base.is_finite() || base < 0.0 {
                continue;
            }
            let mut lee = 0.0_f32;
            let mut local_upwind = base;
            for k in 1..=samples {
                let distance = k as f64 * TRACE_STEP_KM;
                let sx = (x as f64 + distance * dx_per_km).rem_euclid(width as f64);
                let sy = y as f64 + distance * dy_per_km;
                if sy < 0.0 || sy > (height - 1) as f64 {
                    break;
                }
                let upwind = interpolated_height(elevation, sx, sy);
                if !upwind.is_finite() {
                    break;
                }
                if k == (LOCAL_UPLIFT_KM / TRACE_STEP_KM) as usize {
                    local_upwind = upwind.max(0.0);
                }
                if upwind < 0.0 {
                    break;
                } // Ocean resets upwind barrier search.
                let barrier = (upwind - base - 100.0).max(0.0);
                let decay = (-distance / 440.0).exp() as f32;
                lee = lee.max((barrier / 1700.0 * decay).clamp(0.0, 1.0));
            }
            let rise = ((base - local_upwind).max(0.0) / 1400.0).clamp(0.0, 1.0);
            result.cells[y * width + x] = (lee - 0.8 * rise).clamp(-1.0, 1.0);
        }
    }
    result
}

/// Save full-resolution numeric result; a JSON sidecar documents the fictional
/// geospatial convention and the scenario assumption. Not an Earth CRS GeoTIFF.
pub fn export_tiff(grid: &Grid, wind_from: WindFrom, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = TiffEncoder::new(file).map_err(|e| e.to_string())?;
    encoder
        .write_image::<colortype::Gray32Float>(grid.width as u32, grid.height as u32, &grid.cells)
        .map_err(|e| e.to_string())?;
    let metadata = serde_json::json!({
        "layer": "rain_shadow_orographic_exposure_index",
        "units": "dimensionless, signed [-1, 1]",
        "ocean_nodata": "NaN",
        "interpretation": {
            "negative": "windward orographic uplift potential",
            "zero": "weak/neutral terrain signal",
            "positive": "leeward rain-shadow exposure potential"
        },
        "method": "terrain-only upwind barrier proxy v1",
        "wind_from": wind_from.label(),
        "constant_wind_direction": true,
        "assumed_planet_radius_km": PLANET_RADIUS_KM,
        "search_distance_km": MAX_UPWIND_KM,
        "grid_width": grid.width,
        "grid_height": grid.height,
        "lon_pixel_center": "180 - 360*(x+0.5)/width",
        "lat_pixel_center": "-90 + 180*(y+0.5)/height",
        "not_a_precipitation_model": true
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

    fn ridge() -> Grid {
        let mut g = Grid::new(720, 7, 200.0);
        // E-W raster is mirrored: x increases toward geographic west.
        // Mountain rises approaching from western x=240 to crest x=210,
        // then falls toward eastern x=180.
        for y in 0..7 {
            for x in 180..=240 {
                let height = if x <= 210 {
                    (x - 180) as f32 * 100.0
                } else {
                    (240 - x) as f32 * 100.0
                };
                g.cells[y * 720 + x] = 200.0 + height;
            }
        }
        g
    }

    #[test]
    fn west_wind_shadows_east_side_of_ridge() {
        let g = derive(&ridge(), WindFrom::West);
        let lee = g.at(194, 3);
        let windward = g.at(226, 3);
        assert!(lee > 0.1, "expected positive lee exposure, got {lee}");
        assert!(
            windward < 0.0,
            "expected negative windward exposure, got {windward}"
        );
    }

    #[test]
    fn reversing_wind_reverses_lee() {
        let g = derive(&ridge(), WindFrom::East);
        assert!(g.at(226, 3) > 0.1);
        assert!(g.at(194, 3) < 0.0);
    }

    #[test]
    fn ocean_is_nodata() {
        let mut g = ridge();
        g.cells[3 * 720 + 150] = -1000.0;
        let result = derive(&g, WindFrom::West);
        assert!(result.at(150, 3).is_nan());
    }

    #[test]
    fn derivation_is_deterministic_and_grid_aligned() {
        let g = ridge();
        let a = derive(&g, WindFrom::SouthWest);
        let b = derive(&g, WindFrom::SouthWest);
        assert_eq!((a.width, a.height), (720, 7));
        assert_eq!(a.cells, b.cells);
    }

    #[test]
    fn exported_tiff_is_readable_with_metadata() {
        let g = Grid::new(4, 2, 0.25);
        let path = std::env::temp_dir().join(format!("aegaea-rain-shadow-test-{}.tif", std::process::id()));
        export_tiff(&g, WindFrom::West, &path).expect("TIFF export");
        let result = Grid::from_tiff(&path).expect("TIFF round-trip");
        assert_eq!((result.width, result.height), (4, 2));
        assert_eq!(result.cells, g.cells);
        let metadata = std::fs::read_to_string(path.with_extension("json")).expect("sidecar");
        assert!(metadata.contains("not_a_precipitation_model"));
        let _ = std::fs::remove_file(path.with_extension("json"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn old_project_default_is_west() {
        let from: WindFrom = serde_json::from_str("\"west\"").unwrap();
        assert_eq!(from, WindFrom::default());
    }
}
