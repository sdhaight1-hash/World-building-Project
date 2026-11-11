//! Opt-in **geological starting hypothesis** for independent projects.
//!
//! This creates actual, persisted source rasters from spherical plate-site
//! geometry and independent irregular continental-crust nuclei. It never
//! pretends to have solved geological history, subduction polarity, erosion,
//! isostasy or climatic processes. The existing terrain generator consumes
//! these source rasters; it does not paint them for display.
//!
//! This module is intentionally not invoked by blank project creation.
//! Initializing geology requires an explicit user action and refuses to
//! replace any existing source file or already-authored geological raster.
use crate::{
    map_layers::PROVINCE_CLASSES,
    plate_kinematics,
    terrain_generation::{self, TerrainMode},
    world::{CratonInfo, Grid, PlateInfo, Project, ProvinceInfo, World},
};
use serde::Serialize;
use std::{
    f64::consts::{FRAC_PI_2, PI, TAU},
    fs,
    path::Path,
};

const PLATE_COUNT: usize = 15;
const LAND_NUCLEI: usize = 8;

/// Complete, reproducible source geology. All grids share one planet atlas.
#[derive(Debug, Clone)]
pub struct SeededGeology {
    pub seed: u64,
    pub plates: Vec<PlateInfo>,
    pub cratons: Vec<CratonInfo>,
    pub provinces: Vec<ProvinceInfo>,
    pub plate_ids: Grid,
    pub craton_ids: Grid,
    pub province_ids: Grid,
    pub crust_km: Grid,
    pub lithosphere_km: Grid,
    pub heatflow: Grid,
    pub land_fraction: f64,
}

#[derive(Clone, Copy)]
struct Site {
    xyz: [f64; 3],
    radius: f64,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / ((1u64 << 53) as f64))
    }

    fn point(&mut self) -> [f64; 3] {
        let z = 2.0 * self.unit() - 1.0;
        let angle = TAU * self.unit();
        let r = (1.0 - z * z).max(0.0).sqrt();
        [r * angle.cos(), r * angle.sin(), z]
    }
}

fn xyz(lon: f64, lat: f64) -> [f64; 3] {
    let c = lat.cos();
    [c * lon.cos(), c * lon.sin(), lat.sin()]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn field_noise(pos: [f64; 3], seed: u64) -> f32 {
    use crate::terrain_generation::noise3;
    let [x, y, z] = pos.map(|v| v as f32);
    0.085 * noise3(x * 5.0 + 23.0, y * 5.0, z * 5.0, seed)
        + 0.035 * noise3(x * 17.0, y * 17.0 + 41.0, z * 17.0, seed.wrapping_add(1))
        + 0.012 * noise3(x * 47.0, y * 47.0, z * 47.0 + 17.0, seed.wrapping_add(2))
}

pub fn can_initialize(world: &World) -> bool {
    world.plate_ids.is_none()
        && world.craton_ids.is_none()
        && world.province_ids.is_none()
        && world.crust_km.is_none()
        && world.lithosphere_km.is_none()
        && world.heatflow.is_none()
        && world.plates.is_empty()
        && world.cratons.is_empty()
        && world.provinces.is_empty()
}

/// Make spherical plate Voronoi cells, irregular independent continental
/// nuclei, and plausible *hypothetical* crust, thermal and craton inputs.
pub fn generate(world: &World, seed: u64, radius_km: f64) -> Result<SeededGeology, String> {
    if !can_initialize(world) {
        return Err(
            "Source geology already exists; seeded initialization refuses to overwrite it".into(),
        );
    }
    if world.width < 16 || world.height < 8 {
        return Err("Seeded geology requires at least a 16×8 planet grid".into());
    }
    if !radius_km.is_finite() || !(100.0..=100_000.0).contains(&radius_km) {
        return Err("Planet radius is invalid".into());
    }
    let (w, h) = (world.width, world.height);
    let mut rng = Rng(seed);
    // A jittered Fibonacci distribution avoids clustering all plates near
    // ±90°; Voronoi selection uses dot products on the unit sphere, so the
    // longitude seam and physical poles have no special case.
    let azimuth_offset = rng.unit() * TAU;
    let mut sites = Vec::with_capacity(PLATE_COUNT);
    let mut plates = Vec::with_capacity(PLATE_COUNT);
    let mut cratons = Vec::with_capacity(PLATE_COUNT);
    for i in 0..PLATE_COUNT {
        let z = (1.0 - 2.0 * (i as f64 + 0.5) / PLATE_COUNT as f64
            + (rng.unit() - 0.5) / PLATE_COUNT as f64)
            .clamp(-0.99, 0.99);
        let phi = azimuth_offset + i as f64 * 2.399_963_229_728_653 + (rng.unit() - 0.5) * 0.32;
        let planar = (1.0 - z * z).sqrt();
        sites.push(Site {
            xyz: [planar * phi.cos(), planar * phi.sin(), z],
            radius: 0.0,
        });
        let pole = rng.point();
        let signed_rate = if rng.unit() > 0.5 { 1.0 } else { -1.0 } * (0.08 + 0.85 * rng.unit());
        let id = (i + 1) as u8;
        let name = format!("Seed plate {id:02}");
        plates.push(PlateInfo {
            plate_id: id,
            plate: name.clone(),
            color_rgb: format!(
                "{},{},{}",
                75 + i as u16 * 47 % 150,
                75 + i as u16 * 71 % 150,
                85 + i as u16 * 31 % 145,
            ),
            euler_pole_lon_deg: Some(pole[1].atan2(pole[0]).to_degrees()),
            euler_pole_lat_deg: Some(pole[2].asin().to_degrees()),
            signed_rate_deg_per_ma: Some(signed_rate),
        });
        cratons.push(CratonInfo {
            craton_id: id,
            name: format!("Seed shield {id:02}"),
            plate: name,
            basement_age_ga: 2.1 + rng.unit() as f32 * 1.3,
        });
    }
    let mut continental = Vec::with_capacity(LAND_NUCLEI);
    for _ in 0..LAND_NUCLEI {
        continental.push(Site {
            xyz: rng.point(),
            radius: 0.43 + 0.28 * rng.unit(),
        });
    }

    let mut plate_ids = Grid::new(w, h, 0.0);
    let mut continental_signed = vec![0.0f32; w * h];
    let mut continent = vec![false; w * h];
    let mut land_count = 0usize;
    for y in 0..h {
        let lat = -FRAC_PI_2 + PI * (y as f64 + 0.5) / h as f64;
        for x in 0..w {
            let i = y * w + x;
            let lon = PI - TAU * (x as f64 + 0.5) / w as f64;
            let pos = xyz(lon, lat);
            let plate = sites
                .iter()
                .enumerate()
                .max_by(|a, b| dot(a.1.xyz, pos).total_cmp(&dot(b.1.xyz, pos)))
                .map(|(id, _)| id + 1)
                .unwrap();
            plate_ids.cells[i] = plate as f32;
            let signed = continental
                .iter()
                .map(|c| (c.radius - dot(c.xyz, pos).clamp(-1.0, 1.0).acos()) as f32)
                .fold(f32::NEG_INFINITY, f32::max)
                + field_noise(pos, seed);
            continental_signed[i] = signed;
            continent[i] = signed > 0.0;
            land_count += usize::from(continent[i]);
        }
    }
    let land_fraction = land_count as f64 / (w * h) as f64;
    if !(0.10..=0.75).contains(&land_fraction) {
        return Err(format!(
            "Seed produced an extreme land fraction ({land_fraction:.1%}); change seed"
        ));
    }

    // Inferred contact velocities are *instantaneous* from seeded Euler
    // poles. Never infer subducting-side polarity or geological time history.
    let mut geometry = World::empty(world.root.clone());
    geometry.width = w;
    geometry.height = h;
    geometry.plate_ids = Some(plate_ids.clone());
    geometry.plates = plates.clone();
    let kinematics = plate_kinematics::derive(&geometry, radius_km)?;
    let mut province_ids = Grid::new(w, h, 0.0);
    let mut craton_ids = Grid::new(w, h, 0.0);
    let mut crust_km = Grid::new(w, h, 0.0);
    let mut lithosphere_km = Grid::new(w, h, 0.0);
    let mut heatflow = Grid::new(w, h, 0.0);
    for y in 0..h {
        let lat = -FRAC_PI_2 + PI * (y as f64 + 0.5) / h as f64;
        for x in 0..w {
            let i = y * w + x;
            let lon = PI - TAU * (x as f64 + 0.5) / w as f64;
            let pos = xyz(lon, lat);
            let n = field_noise(pos, seed.wrapping_add(103));
            let other_plate = plate_ids.cells[y * w + (x + 1) % w] != plate_ids.cells[i]
                || plate_ids.cells[y * w + (x + w - 1) % w] != plate_ids.cells[i]
                || (y + 1 < h && plate_ids.cells[(y + 1) * w + x] != plate_ids.cells[i])
                || (y > 0 && plate_ids.cells[(y - 1) * w + x] != plate_ids.cells[i]);
            let converging = kinematics
                .as_ref()
                .is_some_and(|g| g.convergence_cm_yr.cells[i] > 1.5);
            let spreading = kinematics
                .as_ref()
                .is_some_and(|g| g.divergence_cm_yr.cells[i] > 1.5);
            let core = continent[i] && continental_signed[i] > 0.18 && !other_plate;
            let province = if continent[i] {
                if other_plate && converging {
                    7
                }
                // collision-proxy orogen
                else if core {
                    4
                }
                // hypothetical ancient shield
                else {
                    3
                } // non-cratonic continent
            } else if other_plate && spreading {
                1 // spreading-axis proxy
            } else {
                0 // ocean basin
            };
            let id = plate_ids.cells[i];
            province_ids.cells[i] = province as f32;
            if core {
                craton_ids.cells[i] = id;
            }
            let (crust, litho, heat) = match province {
                4 => (42.0, 260.0, 38.0),
                7 => (54.0, 160.0, 76.0),
                3 => (35.0, 145.0, 62.0),
                1 => (6.0, 45.0, 145.0),
                _ => (7.5, 85.0, 86.0),
            };
            crust_km.cells[i] = (crust + 9.0 * n).clamp(4.0, 70.0);
            lithosphere_km.cells[i] = (litho + 50.0 * n).clamp(20.0, 320.0);
            heatflow.cells[i] = (heat + 30.0 * n).clamp(15.0, 190.0);
        }
    }

    let provinces = PROVINCE_CLASSES
        .iter()
        .enumerate()
        .map(|(i, name)| ProvinceInfo {
            province_id: i as u8,
            name: (*name).into(),
        })
        .collect();
    Ok(SeededGeology {
        seed,
        plates,
        cratons,
        provinces,
        plate_ids,
        craton_ids,
        province_ids,
        crust_km,
        lithosphere_km,
        heatflow,
        land_fraction,
    })
}

impl SeededGeology {
    /// Replace only an otherwise empty, unmodified world's geological fields.
    pub fn install(self, world: &mut World) -> Result<(), String> {
        if !can_initialize(world) {
            return Err("Existing geology must never be overwritten".into());
        }
        if self.plate_ids.width != world.width || self.plate_ids.height != world.height {
            return Err("Generated raster dimensions do not match the project".into());
        }
        world.plates = self.plates;
        world.cratons = self.cratons;
        world.provinces = self.provinces;
        world.plate_ids = Some(self.plate_ids);
        world.craton_ids = Some(self.craton_ids);
        world.province_ids = Some(self.province_ids);
        world.crust_km = Some(self.crust_km);
        world.lithosphere_km = Some(self.lithosphere_km);
        world.heatflow = Some(self.heatflow);
        Ok(())
    }

    /// Separate, dense float-TIFF sources avoid a half-million JSON overrides.
    /// Refuse overwriting and write all temporary files before renaming them.
    pub fn persist(&self, root: &Path) -> Result<(), String> {
        if !root.is_dir() {
            return Err("Project directory does not exist".into());
        }
        let files = [
            "plate_id.tif",
            "craton_id.tif",
            "province_id.tif",
            "crust_km.tif",
            "lithosphere_km.tif",
            "heatflow_mWm2.tif",
            "plates.csv",
            "cratons.csv",
            "crustal_provinces.csv",
            "geology-scenario.json",
        ];
        for name in files {
            if root.join(name).exists() {
                return Err(format!(
                    "Refusing to overwrite existing geological source: {name}"
                ));
            }
            if root.join(format!("{name}.part")).exists() {
                return Err(format!(
                    "An unfinished geological source exists: {name}.part"
                ));
            }
        }
        let mut temp = Vec::new();
        let write = (|| -> Result<(), String> {
            let grids = [
                ("plate_id.tif", &self.plate_ids),
                ("craton_id.tif", &self.craton_ids),
                ("province_id.tif", &self.province_ids),
                ("crust_km.tif", &self.crust_km),
                ("lithosphere_km.tif", &self.lithosphere_km),
                ("heatflow_mWm2.tif", &self.heatflow),
            ];
            for (name, grid) in grids {
                let path = root.join(format!("{name}.part"));
                temp.push((path.clone(), root.join(name)));
                write_tiff(&path, grid)?;
            }
            for (name, rows) in [
                ("plates.csv", CsvSource::Plates(&self.plates)),
                ("cratons.csv", CsvSource::Cratons(&self.cratons)),
                (
                    "crustal_provinces.csv",
                    CsvSource::Provinces(&self.provinces),
                ),
            ] {
                let path = root.join(format!("{name}.part"));
                temp.push((path.clone(), root.join(name)));
                write_csv(&path, rows)?;
            }
            let path = root.join("geology-scenario.json.part");
            temp.push((path.clone(), root.join("geology-scenario.json")));
            let note = serde_json::json!({
                "generator":"seeded_spherical_geological_hypothesis_v1",
                "seed":self.seed,
                "plate_count":self.plates.len(),
                "continental_nuclei":LAND_NUCLEI,
                "land_pixel_fraction":self.land_fraction,
                "raster_convention":"longitude 180 to -180 as x increases; latitude -90 to 90 as y increases",
                "source_rasters":[
                    "plate_id.tif","province_id.tif","crust_km.tif","craton_id.tif",
                    "lithosphere_km.tif","heatflow_mWm2.tif"
                ],
                "assumptions":"Jittered spherical Voronoi plate geometry, independent noise-warped continental nuclei, plate-local shield IDs, heuristic crust/heat parameters and instantaneous Euler rates. Provenance: generated hypothetical inputs, NOT measured geology or simulated tectonic history.",
                "limitations":"No plate-age history, subduction polarity, mantle convection, crust conservation, isostasy, erosion, or climate inference. Plate-boundary spreading/collision classes are heuristic."
            });
            fs::write(
                &path,
                serde_json::to_vec_pretty(&note).map_err(|e| e.to_string())?,
            )
            .map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(())
        })();
        if let Err(error) = write {
            for (part, _) in &temp {
                let _ = fs::remove_file(part);
            }
            return Err(error);
        }
        let mut committed = Vec::new();
        for (part, final_path) in &temp {
            if let Err(error) = fs::rename(part, final_path) {
                // Roll back ONLY files this operation committed; never
                // remove an existing source introduced by another writer.
                for (tmp, _) in &temp {
                    let _ = fs::remove_file(tmp);
                }
                for path in &committed {
                    let _ = fs::remove_file(path);
                }
                return Err(format!("Unable to commit geological sources: {error}"));
            }
            committed.push(final_path.clone());
        }
        Ok(())
    }
}

/// Initialize source-empty projects only, preserving explicitly blank worlds
/// unless the user invokes the action. Precompute terrain before touching disk.
/// The original flat-ocean baseline is superseded, not added to generated DEM.
pub fn initialize_project(world: &mut World, project: &mut Project) -> Result<f64, String> {
    if !can_initialize(world)
        || !project.layers.grids.is_empty()
        || !project.terrain.overrides.is_empty()
        || world.root.join("elevation_m.tif").exists()
        || project.terrain.base_fill_m.is_some_and(|v| v != -2_000.0)
    {
        return Err("Seeded geology requires an unmodified source-empty project. Existing geology, imported terrain and painted edits are preserved.".into());
    }
    let seeded = generate(
        world,
        project.terrain_recipe.seed,
        project.terrain_recipe.planet_radius_km,
    )?;
    let land_fraction = seeded.land_fraction;
    let mut trial = World::empty(world.root.clone());
    trial.width = world.width;
    trial.height = world.height;
    seeded.clone().install(&mut trial)?;
    let terrain = terrain_generation::generate(&trial, &project.terrain_recipe)?;
    seeded.persist(&world.root)?;
    seeded.install(world)?;
    world.elevation = Some(terrain.elevation);
    world.plate_motion = terrain.kinematics;
    project.terrain.base_fill_m = None;
    project.terrain.grid_size = None;
    project.terrain_mode = TerrainMode::Generated;
    Ok(land_fraction)
}

fn write_tiff(path: &Path, grid: &Grid) -> Result<(), String> {
    use tiff::encoder::{colortype, TiffEncoder};
    let file = fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = TiffEncoder::new(file).map_err(|e| e.to_string())?;
    encoder
        .write_image::<colortype::Gray32Float>(grid.width as u32, grid.height as u32, &grid.cells)
        .map_err(|e| e.to_string())
}

enum CsvSource<'a> {
    Plates(&'a [PlateInfo]),
    Cratons(&'a [CratonInfo]),
    Provinces(&'a [ProvinceInfo]),
}
fn write_csv(path: &Path, rows: CsvSource<'_>) -> Result<(), String> {
    fn records<T: Serialize>(path: &Path, rows: &[T]) -> Result<(), String> {
        let mut w = csv::Writer::from_path(path).map_err(|e| e.to_string())?;
        for row in rows {
            w.serialize(row).map_err(|e| e.to_string())?;
        }
        w.flush().map_err(|e| e.to_string())
    }
    match rows {
        CsvSource::Plates(rows) => records(path, rows),
        CsvSource::Cratons(rows) => records(path, rows),
        CsvSource::Provinces(rows) => records(path, rows),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain_generation::{self, TerrainRecipe};
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn blank(w: usize, h: usize) -> World {
        let mut world = World::empty(PathBuf::from("seeded-fixture"));
        world.width = w;
        world.height = h;
        world
    }

    #[test]
    fn generated_geology_is_deterministic_and_truly_planet_spherical() {
        let w = blank(160, 80);
        let a = generate(&w, 72, 6371.0).unwrap();
        let b = generate(&w, 72, 6371.0).unwrap();
        let c = generate(&w, 73, 6371.0).unwrap();
        assert_eq!(a.plate_ids.cells, b.plate_ids.cells);
        assert_eq!(a.province_ids.cells, b.province_ids.cells);
        assert_eq!(a.crust_km.cells, b.crust_km.cells);
        assert_ne!(a.province_ids.cells, c.province_ids.cells);
        assert!(a.land_fraction > 0.1 && a.land_fraction < 0.75);
        assert!(a.province_ids.cells.iter().any(|&v| v == 0.0));
        assert!(a.province_ids.cells.iter().any(|&v| v >= 3.0));
        assert!(a
            .plate_ids
            .cells
            .iter()
            .all(|&v| (1.0..=PLATE_COUNT as f32).contains(&v)));
        assert!(a.crust_km.cells.iter().all(|&v| (4.0..=70.0).contains(&v)));
        assert!(a
            .heatflow
            .cells
            .iter()
            .all(|&v| (15.0..=190.0).contains(&v)));
        let left = xyz(-PI, 0.7);
        let right = xyz(PI, 0.7);
        assert!(left.iter().zip(right).all(|(a, b)| (a - b).abs() < 1e-12));
    }

    #[test]
    fn generated_sources_drive_elevation_and_survive_native_reopening() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("aegaea-seeded-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let mut w = blank(128, 64);
        w.root = root.clone();
        let seeded = generate(&w, 984, 6371.0).unwrap();
        seeded.persist(&root).unwrap();
        assert!(seeded.persist(&root).unwrap_err().contains("overwrite"));
        let expected = seeded.plate_ids.cells.clone();
        seeded.install(&mut w).unwrap();
        let generated = terrain_generation::generate(&w, &TerrainRecipe::default()).unwrap();
        assert!(generated.elevation.cells.iter().any(|&v| v > 0.0));
        assert!(generated.elevation.cells.iter().any(|&v| v < 0.0));
        let reopened = World::load(root.clone()).unwrap();
        assert_eq!(reopened.plate_ids.as_ref().unwrap().cells, expected);
        assert_eq!(reopened.plates.len(), PLATE_COUNT);
        assert_eq!(reopened.cratons.len(), PLATE_COUNT);
        assert_eq!(reopened.provinces.len(), PROVINCE_CLASSES.len());
        let regenerate =
            terrain_generation::generate(&reopened, &TerrainRecipe::default()).unwrap();
        assert_eq!(generated.elevation.cells, regenerate.elevation.cells);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn blank_project_can_opt_into_seeded_geology_and_reopen_generated_terrain() {
        use crate::world::Project;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "aegaea-seeded-project-{}-{nonce}",
            std::process::id()
        ));
        let (mut world, mut project, path) =
            Project::create_blank(root.clone(), "Genesis").unwrap();
        assert!(world.plate_ids.is_none());
        assert_eq!(project.terrain_mode, TerrainMode::Imported);
        assert!(world
            .elevation
            .as_ref()
            .unwrap()
            .cells
            .iter()
            .all(|&v| v == -2000.0));
        project.terrain_recipe.seed = 1984;
        let fraction = initialize_project(&mut world, &mut project).unwrap();
        assert!(fraction > 0.10 && fraction < 0.75);
        assert_eq!(project.terrain_mode, TerrainMode::Generated);
        assert!(project.terrain.base_fill_m.is_none());
        assert!(world
            .elevation
            .as_ref()
            .unwrap()
            .cells
            .iter()
            .any(|&v| v > 0.0));
        project.save(&path).unwrap();
        let mut reopened = World::load(root.clone()).unwrap();
        let saved_project = Project::load(&path, "fallback").unwrap();
        assert_eq!(saved_project.terrain_mode, TerrainMode::Generated);
        assert_eq!(saved_project.terrain_recipe.seed, 1984);
        let regenerated =
            terrain_generation::generate(&reopened, &saved_project.terrain_recipe).unwrap();
        assert_eq!(
            world.elevation.as_ref().unwrap().cells,
            regenerated.elevation.cells
        );
        reopened.elevation = Some(regenerated.elevation);
        saved_project.terrain.apply(&mut reopened).unwrap();
        assert_eq!(
            world.plate_ids.as_ref().unwrap().cells,
            reopened.plate_ids.as_ref().unwrap().cells
        );
        assert!(initialize_project(&mut reopened, &mut Project::new("again")).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_any_overwrite_even_in_memory() {
        let mut w = blank(64, 32);
        w.plate_ids = Some(Grid::new(64, 32, 1.0));
        assert!(!can_initialize(&w));
        assert!(generate(&w, 12, 6371.0).unwrap_err().contains("exists"));
        let seeded = generate(&blank(64, 32), 12, 6371.0).unwrap();
        assert!(seeded.install(&mut w).unwrap_err().contains("Existing"));
    }
}
