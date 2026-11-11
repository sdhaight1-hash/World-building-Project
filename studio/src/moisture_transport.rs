//! Conservative finite-volume **preview**, not a climate forecast.
//!
//! The authored surface specific humidity (g/kg) is assumed vertically
//! uniform through the atmospheric column. p/g * q is therefore a *proxy*
//! column vapor mass (kg/m²), not observed precipitable water. Prescribed
//! horizontal winds advect this column mass on spherical cell areas.
//! No evaporation, condensation, precipitation, air-mass continuity,
//! vertical physics, pressure tendency, or generated winds are modeled.
use crate::{
    datasets::MonthlyField,
    world::{Grid, World},
};
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

pub const WATER_COLUMN_ID: &str = "transport.vapor_column_proxy";
pub const OPERATOR: &str = "spherical_upwind_column_vapor_transport_v1";
const GRAVITY_M_S2: f64 = 9.80665;
const MAX_COURANT: f64 = 0.75;
const MAX_SUBSTEPS: usize = 512;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub month: u8,
    pub elapsed_seconds: f64,
    pub planet_radius_km: f64,
    pub substeps: usize,
    pub max_outflow_courant: f64,
    pub water_kg_before: f64,
    pub water_kg_after: f64,
    pub relative_mass_error: f64,
    pub minimum_column_kg_m2: f32,
    pub maximum_column_kg_m2: f32,
    pub operator: String,
    pub assumption: String,
}

#[derive(Debug)]
pub struct Preview {
    pub column_kg_m2: Grid,
    pub budget: Budget,
}

#[derive(Debug, Clone, Copy)]
struct Face {
    from: usize,
    to: usize,
    /// Face length (m) times outward upwind velocity (m/s): m²/s.
    swept_m2_s: f64,
}

pub fn missing_sources(world: &World) -> Vec<&'static str> {
    [
        MonthlyField::Pressure,
        MonthlyField::SpecificHumidity,
        MonthlyField::WindEast,
        MonthlyField::WindNorth,
    ]
    .iter()
    .filter_map(|field| {
        (!world.scientific_fields.contains_key(field.id())).then_some(field.id())
    })
    .collect()
}

fn source<'a>(world: &'a World, kind: MonthlyField) -> Result<&'a Grid, String> {
    let grid = world
        .scientific_fields
        .get(kind.id())
        .ok_or_else(|| format!("Missing {} for selected month", kind.name()))?;
    if grid.width != world.width
        || grid.height != world.height
        || grid.cells.len() != world.width * world.height
        || grid.cells.iter().any(|&value| {
            !value.is_finite() || !(kind.bounds()[0]..=kind.bounds()[1]).contains(&value)
        })
    {
        return Err(format!("Invalid shape or values in {}", kind.name()));
    }
    Ok(grid)
}

/// A single explicitly requested interval. Pure: does not edit authored data.
/// Each face transfers equal and opposite water mass, including the periodic
/// longitude seam. Latitude poles have no cross-boundary faces.
pub fn preview(world: &World, radius_km: f64, seconds: f64) -> Result<Preview, String> {
    if world.width == 0 || world.height == 0
        || world.width.checked_mul(world.height).is_none()
    {
        return Err("Transport requires nonempty, representable grid dimensions".into());
    }
    if !radius_km.is_finite() || !(100.0..=100_000.0).contains(&radius_km) {
        return Err("Transport radius must be between 100 and 100,000 km".into());
    }
    if !seconds.is_finite() || !(0.01..=86_400.0).contains(&seconds) {
        return Err("Transport duration must be 0.01–86,400 seconds".into());
    }

    let pressure = source(world, MonthlyField::Pressure)?;
    let humidity = source(world, MonthlyField::SpecificHumidity)?;
    let u = source(world, MonthlyField::WindEast)?;
    let v = source(world, MonthlyField::WindNorth)?;

    let width = world.width;
    let height = world.height;
    let size = width * height;
    let radius = radius_km * 1000.0;
    let dlon = TAU / width as f64;
    let dlat = PI / height as f64;
    let meridian_edge_m = radius * dlat;
    let lat_edge = |y: usize| -FRAC_PI_2 + y as f64 * dlat;
    let area_per_row: Vec<f64> = (0..height)
        .map(|y| {
            radius * radius * dlon
                * (lat_edge(y + 1).sin() - lat_edge(y).sin())
        })
        .collect();

    // f64 internally for global water-budget conservation. Output f32 only
    // after the conservative steps; reported mass is checked on that output.
    let mut mass = vec![0.0; size];
    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            let water_kg_m2 =
                pressure.cells[i] as f64 * 100.0 / GRAVITY_M_S2
                * (humidity.cells[i] as f64 / 1000.0);
            mass[i] = water_kg_m2 * area_per_row[y];
        }
    }
    let before: f64 = mass.iter().sum();

    // Assemble signed upwind flow faces once: Aegaea x increases WEST,
    // while source wind u>0 goes EAST; y increases NORTH with v>0.
    let mut faces = Vec::with_capacity(size.saturating_mul(2));
    let mut outward_per_second = vec![0.0; size];
    for y in 0..height {
        let meridional_face_m = if width > 1 { meridian_edge_m } else { 0.0 };
        for x in 0..width {
            let i = y * width + x;
            if width > 1 {
                let west = y * width + (x + 1) % width;
                let westward = -0.5 * (u.cells[i] as f64 + u.cells[west] as f64);
                if westward != 0.0 {
                    let (from, to) = if westward > 0.0 { (i, west) } else { (west, i) };
                    let sweep = westward.abs() * meridional_face_m;
                    outward_per_second[from] += sweep / area_per_row[y];
                    faces.push(Face { from, to, swept_m2_s: sweep });
                }
            }
            if y + 1 < height {
                let north = i + width;
                let northward = 0.5 * (v.cells[i] as f64 + v.cells[north] as f64);
                if northward != 0.0 {
                    let (from, to) = if northward > 0.0 { (i, north) } else { (north, i) };
                    let sweep = northward.abs() * radius * dlon
                        * lat_edge(y + 1).cos().max(0.0);
                    let donor_y = from / width;
                    outward_per_second[from] += sweep / area_per_row[donor_y];
                    faces.push(Face { from, to, swept_m2_s: sweep });
                }
            }
        }
    }
    let fastest = outward_per_second.into_iter().fold(0.0f64, f64::max);
    let required = (seconds * fastest / MAX_COURANT).ceil().max(1.0);
    if !required.is_finite() || required > MAX_SUBSTEPS as f64 {
        return Err(format!(
            "Transport interval violates stability budget: needs {required:.0} substeps (limit {MAX_SUBSTEPS}). Reduce duration or wind speed."
        ));
    }
    let substeps = required as usize;
    let dt = seconds / substeps as f64;
    let mut changes = vec![0.0; size];
    for _ in 0..substeps {
        changes.fill(0.0);
        for face in &faces {
            let area = area_per_row[face.from / width];
            let flux_kg = mass[face.from] / area * face.swept_m2_s * dt;
            changes[face.from] -= flux_kg;
            changes[face.to] += flux_kg;
        }
        for (i, value) in mass.iter_mut().enumerate() {
            let updated = *value + changes[i];
            if updated < -1.0e-12 * value.max(1.0) {
                return Err("Numerical instability: negative vapor mass after transport".into());
            }
            *value = updated.max(0.0);
        }
    }
    let mut output = Grid::new(width, height, 0.0);
    for (i, v) in output.cells.iter_mut().enumerate() {
        *v = (mass[i] / area_per_row[i / width]) as f32;
        if !v.is_finite() {
            return Err("Transport output exceeded representable float limits".into());
        }
    }
    let after: f64 = output
        .cells
        .iter()
        .enumerate()
        .map(|(i, &water)| water as f64 * area_per_row[i / width])
        .sum();
    let relative_mass_error = (after - before).abs() / before.max(1.0);
    if relative_mass_error > 1.0e-6 {
        return Err(format!(
            "Mass budget exceeded tolerance (relative error {relative_mass_error:.3e})"
        ));
    }
    let minimum = output.cells.iter().copied().fold(f32::INFINITY, f32::min);
    let maximum = output.cells.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    Ok(Preview {
        column_kg_m2: output,
        budget: Budget {
            month: world.active_month,
            elapsed_seconds: seconds,
            planet_radius_km: radius_km,
            substeps,
            max_outflow_courant: dt * fastest,
            water_kg_before: before,
            water_kg_after: after,
            relative_mass_error,
            minimum_column_kg_m2: minimum,
            maximum_column_kg_m2: maximum,
            operator: OPERATOR.into(),
            assumption: "Vertically uniform surface specific humidity over a hydrostatic p/g air column; prescribed winds, no evaporation, rainfall, vertical transport or atmospheric mass continuity. Read-only numerical experiment.".into(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{datasets::DatasetRegistry, world::World};
    use std::path::PathBuf;

    fn fixture(width: usize, height: usize, west_east: f32, south_north: f32) -> World {
        let mut world = World::empty(PathBuf::from("transport-fixture"));
        world.width = width;
        world.height = height;
        let mut registry = DatasetRegistry::default();
        for (field, value) in [
            (MonthlyField::Pressure, 1013.25),
            (MonthlyField::SpecificHumidity, 0.0),
            (MonthlyField::WindEast, west_east),
            (MonthlyField::WindNorth, south_north),
        ] {
            registry.create_month(&mut world, field, 2, value).unwrap();
        }
        world
    }

    #[test]
    fn refuses_missing_fields_or_nonfinite_parameters_without_mutating_world() {
        let mut w = fixture(12, 6, 10.0, 0.0);
        w.scientific_fields.remove(MonthlyField::Pressure.id());
        assert!(preview(&w, 6371.0, 30.0).unwrap_err().contains("pressure"));
        assert!(preview(&w, 6371.0, f64::NAN).is_err());
        w.scientific_fields.insert(MonthlyField::Pressure.id().into(), Grid::new(12, 6, 1013.25));
        assert!(preview(&w, f64::INFINITY, 20.0).is_err());
        w.scientific_fields.get_mut(MonthlyField::WindNorth.id()).unwrap().cells[0] = 300.0;
        assert!(preview(&w, 6371.0, 30.0).is_err());
        assert!(!w.scientific_fields.contains_key(WATER_COLUMN_ID));
    }

    #[test]
    fn zero_wind_preserves_column_and_source_fields() {
        let mut w = fixture(40, 20, 0.0, 0.0);
        w.scientific_fields.get_mut(MonthlyField::SpecificHumidity.id()).unwrap()
            .cells[80] = 30.0;
        let source = w.scientific_fields[MonthlyField::SpecificHumidity.id()].cells.clone();
        let result = preview(&w, 6371.0, 3600.0).unwrap();
        assert_eq!(result.budget.substeps, 1);
        assert_eq!(result.budget.max_outflow_courant, 0.0);
        assert!(result.budget.relative_mass_error < 1.0e-7);
        assert!(result.column_kg_m2.cells[80] > result.column_kg_m2.cells[81]);
        assert_eq!(source, w.scientific_fields[MonthlyField::SpecificHumidity.id()].cells);
    }

    #[test]
    fn positive_eastward_wind_crosses_left_right_longitude_seam() {
        let mut w = fixture(24, 12, 20.0, 0.0);
        let origin = 6 * 24;
        w.scientific_fields.get_mut(MonthlyField::SpecificHumidity.id()).unwrap()
            .cells[origin] = 50.0;
        let result = preview(&w, 6371.0, 120.0).unwrap();
        assert!(result.column_kg_m2.cells[origin + 23] > 0.0,
            "x index decreases eastward, so x=0 moves to x=width-1");
        assert_eq!(result.column_kg_m2.cells[origin + 1], 0.0);
        assert!(result.budget.relative_mass_error < 1.0e-7);
        assert!(result.budget.maximum_column_kg_m2 > result.budget.minimum_column_kg_m2);
    }

    #[test]
    fn polar_boundaries_are_closed_and_north_wind_moves_water_north() {
        let mut w = fixture(32, 16, 0.0, 12.0);
        w.scientific_fields.get_mut(MonthlyField::SpecificHumidity.id()).unwrap()
            .cells[7 * 32 + 8] = 40.0;
        let result = preview(&w, 6371.0, 300.0).unwrap();
        assert!(result.column_kg_m2.cells[8 * 32 + 8] > 0.0);
        assert!(result.budget.relative_mass_error < 1.0e-7);
        assert!(result.column_kg_m2.cells.iter().all(|&v| v >= 0.0));
        // Outward northward velocity cannot lose water through the pole.
        w.scientific_fields.get_mut(MonthlyField::SpecificHumidity.id()).unwrap().cells.fill(0.0);
        w.scientific_fields.get_mut(MonthlyField::SpecificHumidity.id()).unwrap()
            .cells[15 * 32 + 8] = 40.0;
        let at_pole = preview(&w, 6371.0, 300.0).unwrap();
        assert!(at_pole.budget.relative_mass_error < 1.0e-7);
        assert!(at_pole.column_kg_m2.cells[15 * 32 + 8] > 0.0);
    }

    #[test]
    fn divergent_two_component_flow_is_positive_conservative_and_cfl_limited() {
        let mut w = fixture(48, 24, 45.0, -35.0);
        let q = &mut w.scientific_fields.get_mut(MonthlyField::SpecificHumidity.id()).unwrap().cells;
        for (i, value) in q.iter_mut().enumerate() {
            *value = if i % 7 == 0 { 40.0 } else { 5.0 };
        }
        let initial = q.clone();
        let result = preview(&w, 6371.0, 1800.0).unwrap();
        assert!(result.budget.substeps > 1);
        assert!(result.budget.max_outflow_courant <= MAX_COURANT * (1.0 + 1.0e-12));
        assert!(result.budget.relative_mass_error < 1.0e-7);
        assert!(result.column_kg_m2.cells.iter().all(|&v| v.is_finite() && v >= 0.0));
        assert_eq!(w.scientific_fields[MonthlyField::SpecificHumidity.id()].cells, initial);
        let fast = fixture(256, 128, 150.0, 150.0);
        assert!(preview(&fast, 6371.0, 86_400.0)
            .unwrap_err()
            .contains("substeps"));
    }

    #[test]
    fn water_budget_uses_spherical_cell_areas_not_equal_pixel_areas() {
        let mut w = fixture(48, 24, 0.0, 0.0);
        w.scientific_fields
            .get_mut(MonthlyField::SpecificHumidity.id())
            .unwrap()
            .cells
            .fill(10.0);
        let planet_radius_m = 6371.0 * 1000.0;
        let expected = 4.0 * PI * planet_radius_m * planet_radius_m
            * (1013.25 * 100.0 / GRAVITY_M_S2) * 0.01;
        let result = preview(&w, 6371.0, 60.0).unwrap();
        assert!((result.budget.water_kg_before / expected - 1.0).abs() < 1.0e-12);
        assert!((result.budget.water_kg_after / expected - 1.0).abs() < 1.0e-7);
        assert_eq!(result.budget.month, 2);
    }

    #[test]
    fn fixed_pressure_used_only_for_column_conversion() {
        let mut w = fixture(4, 2, 0.0, 0.0);
        w.scientific_fields.get_mut(MonthlyField::SpecificHumidity.id()).unwrap().cells.fill(10.0);
        let pressure = &mut w.scientific_fields.get_mut(MonthlyField::Pressure.id()).unwrap().cells;
        pressure[0] = 500.0;
        pressure[1] = 1000.0;
        let result = preview(&w, 6371.0, 60.0).unwrap();
        let first = result.column_kg_m2.cells[0];
        let second = result.column_kg_m2.cells[1];
        assert!((second / first - 2.0).abs() < 1.0e-6);
    }
}
