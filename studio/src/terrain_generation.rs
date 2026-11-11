//! A geology-constrained planetary elevation *generator*, not a renderer of
//! the inherited DEM. The elevation input is intentionally never read here.
//!
//! Inputs: categorical crustal provinces, plate IDs, crust thickness and
//! optional craton IDs. The province/crust scenarios themselves may contain
//! assumptions inherited from an older terrain; see the provenance manifest.
//!
//! v1 is a reproducible terrain *hypothesis* using tectonic-class forcing,
//! distance-to-coast shelves and seam-safe spherical multiscale variation.
//! It is not a solved tectonics, isostasy, or erosion simulation.
use crate::world::{Grid, World};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, fs::File, path::Path};
use tiff::encoder::{colortype, TiffEncoder};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TerrainMode {
    #[default]
    Auto,
    Imported,
    Generated,
}

impl TerrainMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto (prefer generated geology)",
            Self::Imported => "Legacy imported DEM",
            Self::Generated => "Generated from geology",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TerrainRecipe {
    pub seed: u64,
    pub relief_strength: f32,
    pub planet_radius_km: f64,
}
impl Default for TerrainRecipe {
    fn default() -> Self {
        Self {
            seed: 841_223,
            relief_strength: 1.0,
            planet_radius_km: 6371.0,
        }
    }
}
impl TerrainRecipe {
    pub fn validate(&self) -> Result<(), String> {
        if !self.relief_strength.is_finite() || !(0.0..=2.5).contains(&self.relief_strength) {
            return Err("Terrain relief strength must be finite and between 0 and 2.5".into());
        }
        if !self.planet_radius_km.is_finite()
            || !(100.0..=100_000.0).contains(&self.planet_radius_km)
        {
            return Err("Planet radius must be 100–100,000 km".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct GeneratedTerrain {
    pub elevation: Grid,
    /// Signed tectonic-class forcing, before relief noise and coastal taper.
    pub tectonic_forcing_m: Grid,
    /// 1 = land, 0 = ocean, as constrained by *province class*, not old DEM.
    pub land_mask: Grid,
}

pub fn available(world: &World) -> bool {
    [&world.province_ids, &world.crust_km, &world.plate_ids]
        .iter()
        .all(|field| {
            field.as_ref().is_some_and(|grid| {
                grid.width == world.width
                    && grid.height == world.height
                    && grid.cells.len() == world.width * world.height
            })
        })
}

/// Existing saved projects with terrain edits are not silently retargeted
/// from an old DEM onto a different derived terrain.
pub fn resolve_mode(
    requested: TerrainMode,
    world: &World,
    has_existing_overrides: bool,
) -> TerrainMode {
    match requested {
        TerrainMode::Auto if available(world) && !has_existing_overrides => TerrainMode::Generated,
        TerrainMode::Auto => TerrainMode::Imported,
        explicit => explicit,
    }
}

/// A distance transform with longitude wrap, north/south clamping and a
/// constant grid-cell metric. Planetary kilometre-aware distances are a
/// future spatial-resolution improvement.
fn distance_to(target: &[bool], width: usize, height: usize) -> Vec<u16> {
    let mut distance = vec![u16::MAX; target.len()];
    let mut queue = VecDeque::new();
    for (index, &is_source) in target.iter().enumerate() {
        if is_source {
            distance[index] = 0;
            queue.push_back(index);
        }
    }
    while let Some(index) = queue.pop_front() {
        let x = index % width;
        let y = index / width;
        let next = distance[index].saturating_add(1);
        let west = y * width + (x + 1) % width;
        let east = y * width + (x + width - 1) % width;
        for neighbor in [west, east] {
            if next < distance[neighbor] {
                distance[neighbor] = next;
                queue.push_back(neighbor);
            }
        }
        if y > 0 {
            let neighbor = index - width;
            if next < distance[neighbor] {
                distance[neighbor] = next;
                queue.push_back(neighbor);
            }
        }
        if y + 1 < height {
            let neighbor = index + width;
            if next < distance[neighbor] {
                distance[neighbor] = next;
                queue.push_back(neighbor);
            }
        }
    }
    distance
}

/// Separable box convolution. Two passes produce soft, continuous geological
/// forcing instead of raw category-pixel cliffs. Longitudes wrap at seam.
fn smooth_mask(source: &[f32], width: usize, height: usize, radius: usize) -> Vec<f32> {
    let r = radius
        .min(width.saturating_sub(1))
        .min(height.saturating_sub(1));
    if r == 0 {
        return source.to_vec();
    }
    let span = (2 * r + 1) as f32;
    let mut horizontal = vec![0.0; source.len()];
    for y in 0..height {
        let row = y * width;
        let mut sum = 0.0f32;
        for dx in -(r as isize)..=(r as isize) {
            sum += source[row + dx.rem_euclid(width as isize) as usize];
        }
        for x in 0..width {
            horizontal[row + x] = sum / span;
            let remove = (x + width - (r % width)) % width;
            let add = (x + r + 1) % width;
            sum += source[row + add] - source[row + remove];
        }
    }
    let mut vertical = vec![0.0; source.len()];
    for x in 0..width {
        let mut sum = 0.0f32;
        for dy in -(r as isize)..=(r as isize) {
            let yy = dy.clamp(0, height as isize - 1) as usize;
            sum += horizontal[yy * width + x];
        }
        for y in 0..height {
            vertical[y * width + x] = sum / span;
            let remove = (y as isize - r as isize).clamp(0, height as isize - 1) as usize;
            let add = (y + r + 1).min(height - 1);
            sum += horizontal[add * width + x] - horizontal[remove * width + x];
        }
    }
    vertical
}

fn hash(ix: i32, iy: i32, iz: i32, seed: u64) -> f32 {
    let mut value = seed
        ^ (ix as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (iy as i64 as u64).wrapping_mul(0xbf58476d1ce4e5b9)
        ^ (iz as i64 as u64).wrapping_mul(0x94d049bb133111eb);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58476d1ce4e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d049bb133111eb);
    value ^= value >> 31;
    (value >> 40) as f32 / 8_388_607.5 - 1.0
}
fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Hash-value noise in 3D Cartesian planet coordinates; unlike 2D atlas
/// noise it has no discontinuity at the ±180° meridian or physical poles.
fn noise3(x: f32, y: f32, z: f32, seed: u64) -> f32 {
    let (ix, iy, iz) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    let (fx, fy, fz) = (
        smooth(x - x.floor()),
        smooth(y - y.floor()),
        smooth(z - z.floor()),
    );
    let a = |dx: i32, dy: i32, dz: i32| hash(ix + dx, iy + dy, iz + dz, seed);
    let x00 = mix(a(0, 0, 0), a(1, 0, 0), fx);
    let x10 = mix(a(0, 1, 0), a(1, 1, 0), fx);
    let x01 = mix(a(0, 0, 1), a(1, 0, 1), fx);
    let x11 = mix(a(0, 1, 1), a(1, 1, 1), fx);
    mix(mix(x00, x10, fy), mix(x01, x11, fy), fz)
}

/// Bilinear lookup of the *derived tectonic forcing* at a domain-warped
/// position. Longitude wraps; latitude clamps. The geological input categories
/// themselves remain unchanged.
fn sample_warped(field:&[f32],width:usize,height:usize,x:f32,y:f32)->f32 {
    let x0=x.floor();
    let y0=y.floor();
    let fx=x-x0;
    let fy=y-y0;
    let ax=(x0 as isize).rem_euclid(width as isize) as usize;
    let bx=(ax+1)%width;
    let ay=(y0 as isize).clamp(0,height as isize-1) as usize;
    let by=(ay+1).min(height-1);
    let lo=mix(field[ay*width+ax],field[ay*width+bx],fx);
    let hi=mix(field[by*width+ax],field[by*width+bx],fx);
    mix(lo,hi,fy)
}

fn class_influence(provinces: &[u8], class: u8, width: usize, height: usize) -> Vec<f32> {
    let mask = provinces
        .iter()
        .map(|&v| if v == class { 1.0 } else { 0.0 })
        .collect::<Vec<_>>();
    // Two 7-cell convolutions blend geology at regional, not pixel, scales.
    let once = smooth_mask(&mask, width, height, 3);
    smooth_mask(&once, width, height, 3)
}

/// Returns numeric elevation without *ever* consulting World::elevation.
/// All geometric sources are independently typed geological fields.
pub fn generate(world: &World, recipe: &TerrainRecipe) -> Result<GeneratedTerrain, String> {
    recipe.validate()?;
    if !available(world) {
        return Err("Terrain generation requires aligned province IDs, crust thickness and plate IDs; an old elevation TIFF is NOT required".into());
    }
    let width = world.width;
    let height = world.height;
    let len = width * height;
    if width < 8 || height < 6 {
        return Err("Terrain generation needs a grid at least 8x6".into());
    }
    let province = &world.province_ids.as_ref().unwrap().cells;
    let crust = &world.crust_km.as_ref().unwrap().cells;
    let plate = &world.plate_ids.as_ref().unwrap().cells;
    let mut classes = Vec::with_capacity(len);
    let mut land = Vec::with_capacity(len);
    for i in 0..len {
        let p = province[i];
        if !p.is_finite() || p.fract() != 0.0 || !(0.0..=10.0).contains(&p) {
            return Err(format!("Unrecognized crustal province at pixel {i}: {p}"));
        }
        if !crust[i].is_finite() || !(0.0..=100.0).contains(&crust[i]) {
            return Err(format!("Invalid crust thickness at pixel {i}"));
        }
        if !plate[i].is_finite() || plate[i].fract() != 0.0 || !(0.0..=255.0).contains(&plate[i]) {
            return Err(format!("Invalid plate ID at pixel {i}"));
        }
        classes.push(p as u8);
        land.push(p >= 3.0);
    }
    if land.iter().all(|v| *v) || land.iter().all(|v| !*v) {
        return Err(
            "The reference v1 terrain recipe needs both continental and oceanic provinces".into(),
        );
    }
    let from_land = distance_to(&land, width, height);
    let from_ocean = distance_to(&land.iter().map(|v| !*v).collect::<Vec<_>>(), width, height);
    let mut from_boundary = vec![false; len];
    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            let p = plate[i];
            let other = plate[y * width + (x + 1) % width];
            let up = plate[((y + 1).min(height - 1)) * width + x];
            from_boundary[i] = p != other || p != up;
        }
    }
    let boundary_distance = distance_to(&from_boundary, width, height);
    let orogens=class_influence(&classes,7,width,height);
    let arcs=class_influence(&classes,8,width,height);
    let mut forcing=orogens.iter().zip(&arcs)
        .map(|(&orogen,&arc)|2050.0*orogen+1250.0*arc)
        .collect::<Vec<f32>>();
    // Lower ocean-contact amplitude prevents the classified lines from being
    // reproduced as unbroken, uniformly deep or shallow raster stripes.
    // Domain warp below converts regional class support into terrain structure.
    for (class,magnitude) in [(6,180.0f32),(9,-310.0),(1,750.0),(2,-650.0)] {
        let field=class_influence(&classes,class,width,height);
        for i in 0..len {
            forcing[i]+=field[i]*magnitude;
        }
    }
    let planet_scale = (recipe.planet_radius_km / 6371.0) as f32;
    let mut result = Grid::new(width, height, 0.0);
    let mut land_mask = Grid::new(width, height, 0.0);
    let mut uplift = Grid::new(width, height, 0.0);
    for y in 0..height {
        let lat =
            -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * (y as f64 + 0.5) / height as f64;
        let (cos_lat, sin_lat) = (lat.cos() as f32, lat.sin() as f32);
        for x in 0..width {
            let i = y * width + x;
            let lon =
                std::f64::consts::PI - std::f64::consts::TAU * (x as f64 + 0.5) / width as f64;
            let (sx, sy, sz) = (
                cos_lat * lon.cos() as f32,
                cos_lat * lon.sin() as f32,
                sin_lat,
            );
            let n1 = noise3(
                sx * 8.0 * planet_scale + 40.0,
                sy * 8.0 * planet_scale,
                sz * 8.0 * planet_scale,
                recipe.seed,
            );
            let n2 = noise3(
                sx * 26.0 * planet_scale,
                sy * 26.0 * planet_scale + 17.0,
                sz * 26.0 * planet_scale,
                recipe.seed.wrapping_add(1),
            );
            let n3 = noise3(
                sx * 84.0 * planet_scale,
                sy * 84.0 * planet_scale,
                sz * 84.0 * planet_scale + 19.0,
                recipe.seed.wrapping_add(2),
            );
            let ridged = (1.0
                - noise3(
                    sx * 43.0 * planet_scale + 9.0,
                    sy * 43.0 * planet_scale,
                    sz * 43.0 * planet_scale,
                    recipe.seed.wrapping_add(3),
                )
                .abs())
            .powi(3);
            // Geological support is intentionally a *region*, not a hard
            // raised version of its rectangular/ribbon-shaped class boundary.
            // Seam-safe 3D noise gently bends the tectonic response relative
            // to its source field without changing any crust/plate categories.
            let warp_x=noise3(
                sx*14.0*planet_scale+26.0,sy*14.0*planet_scale,sz*14.0*planet_scale,
                recipe.seed.wrapping_add(4)
            )*7.0;
            let warp_y=noise3(
                sx*14.0*planet_scale,sy*14.0*planet_scale+43.0,sz*14.0*planet_scale,
                recipe.seed.wrapping_add(5)
            )*7.0;
            let px=x as f32+warp_x;
            let py=y as f32+warp_y;
            let orogen_response=sample_warped(&orogens,width,height,px,py);
            let arc_response=sample_warped(&arcs,width,height,px,py);
            let forcing_here=sample_warped(&forcing,width,height,px,py)*recipe.relief_strength;
            let detail=noise3(
                sx*170.0*planet_scale+14.0,sy*170.0*planet_scale,sz*170.0*planet_scale,
                recipe.seed.wrapping_add(6)
            );
            let structure=0.35*n1+0.35*n2+0.20*n3+0.10*detail;
            let near_boundary=(-(boundary_distance[i] as f32)/7.0).exp();
            let shield=world.craton_ids.as_ref()
                .is_some_and(|g|g.width==width && g.height==height && g.at(x,y)>0.0);
            let roughness=if shield {0.65} else {1.0};
            let texture=(620.0+280.0*near_boundary+360.0*orogen_response)
                *structure*roughness*recipe.relief_strength;
            let sea_level = if land[i] {
                land_mask.cells[i] = 1.0;
                let crust_uplift=((crust[i]-35.0)*20.0).clamp(-350.0,650.0);
                let ridge_detail=ridged
                    *(orogen_response*1750.0+arc_response*900.0)
                    *recipe.relief_strength;
                let base=550.0
                    +crust_uplift
                    +if shield {100.0} else {0.0}
                    +forcing_here
                    +texture
                    +ridge_detail;
                let coast = 1.0 - (-(from_ocean[i] as f32) / 3.2).exp();
                (55.0 + (base - 55.0) * coast).clamp(10.0, 8000.0)
            } else {
                let ocean_base =
                    -4450.0 + forcing_here + (350.0 * structure) * recipe.relief_strength;
                let shelf = 1.0 - (-(from_land[i] as f32) / 5.8).exp();
                (-60.0 + (ocean_base + 60.0) * shelf).clamp(-10500.0, -20.0)
            };
            result.cells[i] = sea_level;
            uplift.cells[i] = forcing_here;
        }
    }
    // Every longitude shares the same geometric pole. Average the narrow
    // polar row to enforce a single height; original geologic inputs remain.
    for row in [0, height - 1] {
        let start = row * width;
        let mean = result.cells[start..start + width].iter().sum::<f32>() / width as f32;
        result.cells[start..start + width].fill(mean);
        let up_mean = uplift.cells[start..start + width].iter().sum::<f32>() / width as f32;
        uplift.cells[start..start + width].fill(up_mean);
    }
    Ok(GeneratedTerrain {
        elevation: result,
        tectonic_forcing_m: uplift,
        land_mask,
    })
}

fn write_float_tiff(path: &Path, grid: &Grid) -> Result<(), String> {
    let file = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = TiffEncoder::new(file).map_err(|e| e.to_string())?;
    encoder
        .write_image::<colortype::Gray32Float>(grid.width as u32, grid.height as u32, &grid.cells)
        .map_err(|e| e.to_string())
}

/// Numeric source-independent outputs and their explicit provenance. Never
/// write inside the immutable data/aegaea source directory.
pub fn export_generation(
    result: &GeneratedTerrain,
    recipe: &TerrainRecipe,
    dir: &Path,
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    write_float_tiff(&dir.join("generated-elevation_m.tif"), &result.elevation)?;
    write_float_tiff(
        &dir.join("tectonic-forcing_m.tif"),
        &result.tectonic_forcing_m,
    )?;
    write_float_tiff(&dir.join("geology-land-mask.tif"), &result.land_mask)?;
    let metadata = serde_json::json!({
        "generator":"geology-constrained-terrain-v1",
        "recipe":recipe,
        "outputs":{
            "generated-elevation_m.tif":"metres relative to fictional sea level",
            "tectonic-forcing_m.tif":"hypothetical province-class elevation contribution, metres",
            "geology-land-mask.tif":"categorical 0 ocean, 1 continent"
        },
        "input_fields":["province_id.tif","crust_km.tif","plate_id.tif",
                        "optional craton_id.tif"],
        "old_elevation_used":false,
        "geometry":"native mirrored/inverted longitude +180 to -180, latitude -90 to +90",
        "status":"reproducible geology-constrained heuristic, NOT a validated dynamic tectonic or erosion simulation",
        "important_limitation":"Source crust/province classes may themselves have inherited assumptions from the legacy elevation generator"
    });
    std::fs::write(
        dir.join("generation.json"),
        serde_json::to_vec_pretty(&metadata).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    fn fixture() -> World {
        let mut w = World::empty(PathBuf::from("not-aegaea"));
        w.width = 96;
        w.height = 48;
        w.province_ids = Some(Grid::new(96, 48, 0.0));
        w.crust_km = Some(Grid::new(96, 48, 7.0));
        w.plate_ids = Some(Grid::new(96, 48, 0.0));
        for y in 8..40 {
            for x in 20..67 {
                let i = y * 96 + x;
                w.province_ids.as_mut().unwrap().cells[i] = if x == 46 { 7.0 } else { 3.0 };
                w.crust_km.as_mut().unwrap().cells[i] = if x == 46 { 45.0 } else { 35.0 };
                w.plate_ids.as_mut().unwrap().cells[i] = if x < 48 { 0.0 } else { 1.0 };
            }
        }
        w
    }

    #[test]
    fn creates_terrain_with_no_old_elevation_file() {
        let world = fixture();
        assert!(world.elevation.is_none());
        let result = generate(&world, &TerrainRecipe::default()).unwrap();
        assert_eq!((result.elevation.width, result.elevation.height), (96, 48));
        assert!(result.elevation.cells.iter().any(|x| *x > 0.0));
        assert!(result.elevation.cells.iter().any(|x| *x < 0.0));
        assert!(result
            .land_mask
            .cells
            .iter()
            .all(|v| *v == 0.0 || *v == 1.0));
    }

    #[test]
    fn previous_height_field_cannot_affect_generated_result() {
        let w = fixture();
        let mut tampered = fixture();
        tampered.elevation = Some(Grid::new(96, 48, 99_999.0));
        let a = generate(&w, &TerrainRecipe::default()).unwrap();
        let b = generate(&tampered, &TerrainRecipe::default()).unwrap();
        assert_eq!(a.elevation.cells, b.elevation.cells);
        assert_eq!(a.tectonic_forcing_m.cells, b.tectonic_forcing_m.cells);
    }

    #[test]
    fn seed_is_deterministic_and_changes_detail_not_continent_mask() {
        let w = fixture();
        let a = generate(&w, &TerrainRecipe::default()).unwrap();
        let b = generate(&w, &TerrainRecipe::default()).unwrap();
        assert_eq!(a.elevation.cells, b.elevation.cells);
        let mut options = TerrainRecipe::default();
        options.seed += 1;
        let c = generate(&w, &options).unwrap();
        assert_ne!(a.elevation.cells, c.elevation.cells);
        assert_eq!(a.land_mask.cells, c.land_mask.cells);
    }

    #[test]
    fn mountain_province_generates_uplift_and_shelves() {
        let w = fixture();
        let r = generate(
            &w,
            &TerrainRecipe {
                relief_strength: 1.0,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(r.tectonic_forcing_m.at(46, 22) > r.tectonic_forcing_m.at(26, 22));
        assert!(r.elevation.at(46, 22) > r.elevation.at(26, 22));
        assert!(
            r.elevation.at(19, 22) > r.elevation.at(2, 22),
            "ocean adjacent to continent must be shallower than deep basin"
        );
    }

    #[test]
    fn generated_poles_converge_and_elevations_are_finite() {
        let r = generate(&fixture(), &TerrainRecipe::default()).unwrap();
        assert!(r.elevation.cells.iter().all(|v| v.is_finite()));
        for row in [0, r.elevation.height - 1] {
            assert!(
                r.elevation.cells[row * r.elevation.width..(row + 1) * r.elevation.width]
                    .iter()
                    .all(|v| *v == r.elevation.at(0, row))
            );
        }
    }

    #[test]
    fn auto_uses_generated_geology_without_overriding_old_projects() {
        let w = fixture();
        assert_eq!(
            resolve_mode(TerrainMode::Auto, &w, false),
            TerrainMode::Generated
        );
        assert_eq!(
            resolve_mode(TerrainMode::Auto, &w, true),
            TerrainMode::Imported
        );
        assert_eq!(
            resolve_mode(TerrainMode::Imported, &w, false),
            TerrainMode::Imported
        );
    }

    #[test]
    fn planet_scale_changes_feature_width_and_seed_overflow_is_safe() {
        let w = fixture();
        let mut recipe = TerrainRecipe::default();
        recipe.seed = u64::MAX;
        let a = generate(&w, &recipe).unwrap();
        recipe.planet_radius_km = 5000.0;
        let b = generate(&w, &recipe).unwrap();
        assert_ne!(a.elevation.cells, b.elevation.cells);
    }
    #[test]
    fn exports_numeric_fields_and_reproducible_provenance() {
        let recipe = TerrainRecipe::default();
        let result = generate(&fixture(), &recipe).unwrap();
        let dir = std::env::temp_dir().join(format!("aegaea-generator-{}", std::process::id()));
        export_generation(&result, &recipe, &dir).unwrap();
        let reread = Grid::from_tiff(&dir.join("generated-elevation_m.tif")).unwrap();
        assert_eq!(result.elevation.cells, reread.cells);
        let manifest = std::fs::read_to_string(dir.join("generation.json")).unwrap();
        assert!(manifest.contains("\"old_elevation_used\": false"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reference_world_can_generate_without_any_elevation_grid() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/aegaea/tectonics/v1");
        let mut world = World::load(root).unwrap();
        world.elevation = None; // The primary source DEM is intentionally absent.
        let r = generate(&world, &TerrainRecipe::default()).unwrap();
        assert_eq!((r.elevation.width, r.elevation.height), (1672, 941));
        assert!(r.elevation.cells.iter().any(|v| *v > 3000.0));
        assert!(r.elevation.cells.iter().any(|v| *v < -3000.0));
    }
}
