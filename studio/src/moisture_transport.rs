//! Coupled spherical finite-volume **mass-continuity preview**, not a weather forecast.
//!
//! A hydrostatic p/g *total moist-air column* and its water-vapor fraction are
//! transported by the SAME upwind face flux. This conserves total air, dry
//! air and water mass, with a positive and bounded mixing ratio, while letting
//! diagnostic surface pressure respond to horizontal convergence/divergence.
//!
//! Prescribed wind fields remain FIXED. No momentum equation, Coriolis force,
//! vertical structure, terrain coupling, heat, precipitation, evaporation,
//! or thermodynamics is solved. These outputs are ephemeral, never authored
//! source data.
use crate::{
    datasets::{MonthlyField, Provenance},
    world::{Grid, World},
};
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

pub const WATER_COLUMN_ID: &str = "transport.vapor_column_proxy";
pub const PRESSURE_PREVIEW_ID: &str = "transport.surface_pressure_preview";
pub const HUMIDITY_PREVIEW_ID: &str = "transport.specific_humidity_preview";
pub const OPERATOR: &str = "spherical_upwind_moist_air_and_vapor_continuity_v2";
const MAX_COURANT: f64 = 0.75;
const MAX_SUBSTEPS: usize = 512;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub month: u8,
    pub elapsed_seconds: f64,
    pub planet_radius_km: f64,
    pub surface_gravity_m_s2: f64,
    pub substeps: usize,
    pub max_outflow_courant: f64,
    pub air_kg_before: f64,
    pub air_kg_after: f64,
    pub air_relative_mass_error: f64,
    pub water_kg_before: f64,
    pub water_kg_after: f64,
    pub relative_mass_error: f64,
    pub minimum_pressure_hpa: f32,
    pub maximum_pressure_hpa: f32,
    pub minimum_humidity_g_kg: f32,
    pub maximum_humidity_g_kg: f32,
    pub minimum_column_kg_m2: f32,
    pub maximum_column_kg_m2: f32,
    pub operator: String,
    pub assumption: String,
}

#[derive(Debug)]
pub struct Preview {
    pub column_kg_m2: Grid,
    pub pressure_hpa: Grid,
    pub specific_humidity_g_kg: Grid,
    pub budget: Budget,
}

#[derive(Debug, Clone, Copy)]
struct Face {
    from: usize,
    to: usize,
    /// Face length (m) times outward upwind velocity (m/s): m²/s.
    swept_m2_s: f64,
}

/// An explicit preview is ephemeral: register only computed fields, never the
/// dataset registry or project serialization. Replacing a prior preview is safe.
pub fn publish(world: &mut World, preview: Preview) {
    let inputs = vec![
        MonthlyField::Pressure.id().into(),
        MonthlyField::SpecificHumidity.id().into(),
        MonthlyField::WindEast.id().into(),
        MonthlyField::WindNorth.id().into(),
        "planet.radius".into(),
        "planet.surface_gravity".into(),
    ];
    for (id, grid) in [
        (WATER_COLUMN_ID, preview.column_kg_m2),
        (PRESSURE_PREVIEW_ID, preview.pressure_hpa),
        (HUMIDITY_PREVIEW_ID, preview.specific_humidity_g_kg),
    ] {
        world.scientific_fields.insert(id.into(), grid);
        world.scientific_provenance.insert(
            id.into(),
            Provenance::Derived {
                operator: OPERATOR.into(),
                inputs: inputs.clone(),
            },
        );
    }
    world.moisture_transport_budget = Some(preview.budget);
}

/// Input reactivation clears these results; physical-parameter edits clear them
/// explicitly to prevent showing maps from an obsolete radius or gravity.
pub fn clear_preview(world: &mut World) {
    for id in [WATER_COLUMN_ID, PRESSURE_PREVIEW_ID, HUMIDITY_PREVIEW_ID] {
        world.scientific_fields.remove(id);
        world.scientific_provenance.remove(id);
    }
    world.moisture_transport_budget = None;
}

pub fn missing_sources(world: &World) -> Vec<&'static str> {
    [
        MonthlyField::Pressure,
        MonthlyField::SpecificHumidity,
        MonthlyField::WindEast,
        MonthlyField::WindNorth,
    ]
    .iter()
    .filter_map(|field| (!world.scientific_fields.contains_key(field.id())).then_some(field.id()))
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

/// One read-only, explicitly requested time interval. Moist-air surface
/// pressure is inferred from air-column mass p = g*A, not advanced separately.
/// Fluxes transport air mass AND its source-cell water fraction together.
pub fn preview(
    world: &World,
    radius_km: f64,
    gravity_m_s2: f64,
    seconds: f64,
) -> Result<Preview, String> {
    if world.width == 0 || world.height == 0 || world.width.checked_mul(world.height).is_none() {
        return Err("Transport requires nonempty, representable grid dimensions".into());
    }
    if !radius_km.is_finite() || !(100.0..=100_000.0).contains(&radius_km) {
        return Err("Transport radius must be between 100 and 100,000 km".into());
    }
    if !gravity_m_s2.is_finite() || !(0.1..=100.0).contains(&gravity_m_s2) {
        return Err("Surface gravity must be 0.1–100 m/s²".into());
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
        .map(|y| radius * radius * dlon * (lat_edge(y + 1).sin() - lat_edge(y).sin()))
        .collect();

    // f64 internal state: kg of moist air and kg of water vapor per cell.
    // Vapor is carried with the same donor upwind velocity as its carrier air.
    let mut air_mass = vec![0.0f64; size];
    let mut vapor_mass = vec![0.0f64; size];
    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            air_mass[i] = pressure.cells[i] as f64 * 100.0 / gravity_m_s2 * area_per_row[y];
            vapor_mass[i] = air_mass[i] * humidity.cells[i] as f64 / 1000.0;
        }
    }
    let air_before: f64 = air_mass.iter().sum();
    let water_before: f64 = vapor_mass.iter().sum();

    // Aegaea's map x increases WEST, whereas u>0 points EAST; y points NORTH.
    // Every interface has one directed upwind face; no polar boundary face.
    let mut faces = Vec::with_capacity(size.saturating_mul(2));
    let mut outward_per_second = vec![0.0f64; size];
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
                    faces.push(Face {
                        from,
                        to,
                        swept_m2_s: sweep,
                    });
                }
            }
            if y + 1 < height {
                let north = i + width;
                let northward = 0.5 * (v.cells[i] as f64 + v.cells[north] as f64);
                if northward != 0.0 {
                    let (from, to) = if northward > 0.0 {
                        (i, north)
                    } else {
                        (north, i)
                    };
                    let sweep = northward.abs() * radius * dlon * lat_edge(y + 1).cos().max(0.0);
                    outward_per_second[from] += sweep / area_per_row[from / width];
                    faces.push(Face {
                        from,
                        to,
                        swept_m2_s: sweep,
                    });
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
    let mut air_change = vec![0.0; size];
    let mut vapor_change = vec![0.0; size];
    for _ in 0..substeps {
        air_change.fill(0.0);
        vapor_change.fill(0.0);
        for face in &faces {
            let fraction = face.swept_m2_s * dt / area_per_row[face.from / width];
            let moved_air = air_mass[face.from] * fraction;
            let moved_vapor = vapor_mass[face.from] * fraction;
            air_change[face.from] -= moved_air;
            air_change[face.to] += moved_air;
            vapor_change[face.from] -= moved_vapor;
            vapor_change[face.to] += moved_vapor;
        }
        for i in 0..size {
            let next_air = air_mass[i] + air_change[i];
            let next_vapor = vapor_mass[i] + vapor_change[i];
            // Same upwind operator retains 0 <= vapor <= air by construction.
            if !next_air.is_finite()
                || !next_vapor.is_finite()
                || next_air <= 0.0
                || next_vapor < -1.0e-12 * next_air
                || next_vapor > next_air * (1.0 + 1.0e-12)
            {
                return Err("Numerical instability: invalid moist-air or water mass".into());
            }
            air_mass[i] = next_air;
            vapor_mass[i] = next_vapor.max(0.0);
        }
    }

    let mut column = Grid::new(width, height, 0.0);
    let mut pressure_out = Grid::new(width, height, 0.0);
    let mut humidity_out = Grid::new(width, height, 0.0);
    for i in 0..size {
        let area = area_per_row[i / width];
        column.cells[i] = (vapor_mass[i] / area) as f32;
        pressure_out.cells[i] = (air_mass[i] / area * gravity_m_s2 / 100.0) as f32;
        humidity_out.cells[i] = (1000.0 * vapor_mass[i] / air_mass[i]) as f32;
        if !column.cells[i].is_finite()
            || !pressure_out.cells[i].is_finite()
            || !humidity_out.cells[i].is_finite()
        {
            return Err("Transport output exceeded representable float limits".into());
        }
    }

    // Compute budgets from the *published f32 rasters*, not hidden f64 state.
    let air_after: f64 = pressure_out
        .cells
        .iter()
        .enumerate()
        .map(|(i, &p)| p as f64 * 100.0 / gravity_m_s2 * area_per_row[i / width])
        .sum();
    let water_after: f64 = column
        .cells
        .iter()
        .enumerate()
        .map(|(i, &w)| w as f64 * area_per_row[i / width])
        .sum();
    let air_error = (air_after - air_before).abs() / air_before.max(1.0);
    let water_error = (water_after - water_before).abs() / water_before.max(1.0);
    if air_error > 1.0e-6 || water_error > 1.0e-6 {
        return Err(format!(
            "Mass budget exceeded tolerance: air {air_error:.3e}, water {water_error:.3e}"
        ));
    }
    let range = |g: &Grid| -> (f32, f32) {
        (
            g.cells.iter().copied().fold(f32::INFINITY, f32::min),
            g.cells.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        )
    };
    let (min_water, max_water) = range(&column);
    let (min_pressure, max_pressure) = range(&pressure_out);
    let (min_humidity, max_humidity) = range(&humidity_out);

    Ok(Preview {
        column_kg_m2: column,
        pressure_hpa: pressure_out,
        specific_humidity_g_kg: humidity_out,
        budget: Budget {
            month: world.active_month,
            elapsed_seconds: seconds,
            planet_radius_km: radius_km,
            surface_gravity_m_s2: gravity_m_s2,
            substeps,
            max_outflow_courant: dt * fastest,
            air_kg_before: air_before,
            air_kg_after: air_after,
            air_relative_mass_error: air_error,
            water_kg_before: water_before,
            water_kg_after: water_after,
            relative_mass_error: water_error,
            minimum_pressure_hpa: min_pressure,
            maximum_pressure_hpa: max_pressure,
            minimum_humidity_g_kg: min_humidity,
            maximum_humidity_g_kg: max_humidity,
            minimum_column_kg_m2: min_water,
            maximum_column_kg_m2: max_water,
            operator: OPERATOR.into(),
            assumption: "Hydrostatic total moist-air column p/g with vertically uniform specific humidity; both advected by fixed prescribed winds. No prognostic momentum, Coriolis, vertical transport, evaporation, condensation, precipitation, radiation or topography. Diagnostic pressure tendency only.".into(),
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
        assert!(preview(&w, 6371.0, 9.80665, 30.0)
            .unwrap_err()
            .contains("pressure"));
        assert!(preview(&w, 6371.0, 9.80665, f64::NAN).is_err());
        assert!(preview(&w, 6371.0, 0.0, 30.0).is_err());
        w.scientific_fields.insert(
            MonthlyField::Pressure.id().into(),
            Grid::new(12, 6, 1013.25),
        );
        assert!(preview(&w, f64::INFINITY, 9.80665, 20.0).is_err());
        w.scientific_fields
            .get_mut(MonthlyField::WindNorth.id())
            .unwrap()
            .cells[0] = 300.0;
        assert!(preview(&w, 6371.0, 9.80665, 30.0).is_err());
        assert!(!w.scientific_fields.contains_key(WATER_COLUMN_ID));
    }

    #[test]
    fn zero_wind_preserves_column_and_source_fields() {
        let mut w = fixture(40, 20, 0.0, 0.0);
        w.scientific_fields
            .get_mut(MonthlyField::SpecificHumidity.id())
            .unwrap()
            .cells[80] = 30.0;
        let source = w.scientific_fields[MonthlyField::SpecificHumidity.id()]
            .cells
            .clone();
        let result = preview(&w, 6371.0, 9.80665, 3600.0).unwrap();
        assert_eq!(result.budget.substeps, 1);
        assert_eq!(result.budget.max_outflow_courant, 0.0);
        assert!(result.budget.air_relative_mass_error < 1.0e-7);
        assert!(result.pressure_hpa.cells.iter().all(|&v| v == 1013.25));
        assert!(result.budget.relative_mass_error < 1.0e-7);
        assert!(result.column_kg_m2.cells[80] > result.column_kg_m2.cells[81]);
        assert_eq!(
            source,
            w.scientific_fields[MonthlyField::SpecificHumidity.id()].cells
        );
    }

    #[test]
    fn positive_eastward_wind_crosses_left_right_longitude_seam() {
        let mut w = fixture(24, 12, 20.0, 0.0);
        let origin = 6 * 24;
        w.scientific_fields
            .get_mut(MonthlyField::SpecificHumidity.id())
            .unwrap()
            .cells[origin] = 50.0;
        let result = preview(&w, 6371.0, 9.80665, 120.0).unwrap();
        assert!(
            result.column_kg_m2.cells[origin + 23] > 0.0,
            "x index decreases eastward, so x=0 moves to x=width-1"
        );
        assert_eq!(result.column_kg_m2.cells[origin + 1], 0.0);
        assert!(result.budget.relative_mass_error < 1.0e-7);
        assert!(result.budget.maximum_column_kg_m2 > result.budget.minimum_column_kg_m2);
    }

    #[test]
    fn polar_boundaries_are_closed_and_north_wind_moves_water_north() {
        let mut w = fixture(32, 16, 0.0, 12.0);
        w.scientific_fields
            .get_mut(MonthlyField::SpecificHumidity.id())
            .unwrap()
            .cells[7 * 32 + 8] = 40.0;
        let result = preview(&w, 6371.0, 9.80665, 300.0).unwrap();
        assert!(result.column_kg_m2.cells[8 * 32 + 8] > 0.0);
        assert!(result.budget.relative_mass_error < 1.0e-7);
        assert!(result.column_kg_m2.cells.iter().all(|&v| v >= 0.0));
        // Outward northward velocity cannot lose water through the pole.
        w.scientific_fields
            .get_mut(MonthlyField::SpecificHumidity.id())
            .unwrap()
            .cells
            .fill(0.0);
        w.scientific_fields
            .get_mut(MonthlyField::SpecificHumidity.id())
            .unwrap()
            .cells[15 * 32 + 8] = 40.0;
        let at_pole = preview(&w, 6371.0, 9.80665, 300.0).unwrap();
        assert!(at_pole.budget.relative_mass_error < 1.0e-7);
        assert!(at_pole.column_kg_m2.cells[15 * 32 + 8] > 0.0);
    }

    #[test]
    fn divergent_two_component_flow_is_positive_conservative_and_cfl_limited() {
        let mut w = fixture(48, 24, 45.0, -35.0);
        let q = &mut w
            .scientific_fields
            .get_mut(MonthlyField::SpecificHumidity.id())
            .unwrap()
            .cells;
        for (i, value) in q.iter_mut().enumerate() {
            *value = if i % 7 == 0 { 40.0 } else { 5.0 };
        }
        let initial = q.clone();
        let result = preview(&w, 6371.0, 9.80665, 1800.0).unwrap();
        assert!(result.budget.substeps > 1);
        assert!(result.budget.max_outflow_courant <= MAX_COURANT * (1.0 + 1.0e-12));
        assert!(result.budget.relative_mass_error < 1.0e-7);
        assert!(result
            .column_kg_m2
            .cells
            .iter()
            .all(|&v| v.is_finite() && v >= 0.0));
        assert_eq!(
            w.scientific_fields[MonthlyField::SpecificHumidity.id()].cells,
            initial
        );
        let fast = fixture(256, 128, 150.0, 150.0);
        assert!(preview(&fast, 6371.0, 9.80665, 86_400.0)
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
        let expected =
            4.0 * PI * planet_radius_m * planet_radius_m * (1013.25 * 100.0 / 9.80665) * 0.01;
        let result = preview(&w, 6371.0, 9.80665, 60.0).unwrap();
        assert!((result.budget.water_kg_before / expected - 1.0).abs() < 1.0e-12);
        assert!((result.budget.water_kg_after / expected - 1.0).abs() < 1.0e-7);
        assert_eq!(result.budget.month, 2);
    }

    #[test]
    fn pressure_responds_to_advection_and_air_mass_is_conserved() {
        let mut w = fixture(32, 16, 40.0, 0.0);
        let source = &mut w
            .scientific_fields
            .get_mut(MonthlyField::Pressure.id())
            .unwrap()
            .cells;
        source[8 * 32] = 1150.0;
        let input = source.clone();
        let result = preview(&w, 6371.0, 9.80665, 1200.0).unwrap();
        // Positive geographic east is decreasing Aegaea x.
        assert!(result.pressure_hpa.cells[8 * 32] < 1150.0);
        assert!(result.pressure_hpa.cells[8 * 32 + 31] > 1013.25);
        assert!(result.pressure_hpa.cells[8 * 32 + 1] == 1013.25);
        assert!(result.budget.air_relative_mass_error < 1.0e-7);
        assert!(result.budget.relative_mass_error < 1.0e-7);
        assert_eq!(
            w.scientific_fields[MonthlyField::Pressure.id()].cells,
            input
        );
    }

    #[test]
    fn divergent_prescribed_flow_changes_pressure_but_not_uniform_mixing_ratio() {
        let mut w = fixture(32, 16, 0.0, 0.0);
        w.scientific_fields
            .get_mut(MonthlyField::SpecificHumidity.id())
            .unwrap()
            .cells
            .fill(12.0);
        // Divergence from prescribed eastward velocity near x=5.
        let wind = &mut w
            .scientific_fields
            .get_mut(MonthlyField::WindEast.id())
            .unwrap()
            .cells;
        for y in 0..16 {
            wind[y * 32 + 5] = 50.0;
        }
        let result = preview(&w, 6371.0, 9.80665, 1800.0).unwrap();
        assert!(result
            .pressure_hpa
            .cells
            .iter()
            .any(|p| (*p - 1013.25).abs() > 0.01));
        assert!(result
            .specific_humidity_g_kg
            .cells
            .iter()
            .all(|q| (*q - 12.0).abs() < 0.0001));
        assert!(result.budget.air_relative_mass_error < 1.0e-7);
        assert!(result.budget.relative_mass_error < 1.0e-7);
        assert!(result.budget.minimum_pressure_hpa > 0.0);
    }

    #[test]
    fn tracer_stays_bounded_even_when_moist_air_columns_diverge() {
        let mut w = fixture(48, 24, -60.0, 40.0);
        w.scientific_fields
            .get_mut(MonthlyField::SpecificHumidity.id())
            .unwrap()
            .cells
            .iter_mut()
            .enumerate()
            .for_each(|(i, q)| {
                *q = if i % 3 == 0 { 60.0 } else { 0.0 };
            });
        let result = preview(&w, 6371.0, 9.80665, 1800.0).unwrap();
        assert!(result
            .specific_humidity_g_kg
            .cells
            .iter()
            .all(|&q| q >= 0.0 && q <= 60.001));
        assert!(result
            .pressure_hpa
            .cells
            .iter()
            .all(|&p| p > 0.0 && p.is_finite()));
        assert!(result.budget.air_relative_mass_error < 1.0e-7);
        assert!(result.budget.relative_mass_error < 1.0e-7);
    }

    #[test]
    fn gravity_changes_column_mass_but_not_initial_pressure() {
        let mut w = fixture(8, 4, 0.0, 0.0);
        w.scientific_fields
            .get_mut(MonthlyField::SpecificHumidity.id())
            .unwrap()
            .cells
            .fill(20.0);
        let earth = preview(&w, 6371.0, 9.80665, 60.0).unwrap();
        let low_g = preview(&w, 6371.0, 3.71, 60.0).unwrap();
        assert!(
            (low_g.column_kg_m2.at(0, 0) / earth.column_kg_m2.at(0, 0) - (9.80665 / 3.71) as f32)
                .abs()
                < 1.0e-5
        );
        assert_eq!(earth.pressure_hpa.at(0, 0), 1013.25);
        assert_eq!(low_g.pressure_hpa.at(0, 0), 1013.25);
        assert_eq!(low_g.budget.surface_gravity_m_s2, 3.71);
        assert!(low_g.budget.air_relative_mass_error < 1.0e-7);
    }

    #[test]
    fn fixed_pressure_used_only_for_column_conversion() {
        let mut w = fixture(4, 2, 0.0, 0.0);
        w.scientific_fields
            .get_mut(MonthlyField::SpecificHumidity.id())
            .unwrap()
            .cells
            .fill(10.0);
        let pressure = &mut w
            .scientific_fields
            .get_mut(MonthlyField::Pressure.id())
            .unwrap()
            .cells;
        pressure[0] = 500.0;
        pressure[1] = 1000.0;
        let result = preview(&w, 6371.0, 9.80665, 60.0).unwrap();
        let first = result.column_kg_m2.cells[0];
        let second = result.column_kg_m2.cells[1];
        assert!((second / first - 2.0).abs() < 1.0e-6);
    }
}
