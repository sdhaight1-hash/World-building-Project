//! Experimental linearized barotropic wind tendency over conservative air/vapor columns.
//!
//! This is NOT the rotating shallow-water equations in full: no momentum
//! advection, metric/curvature terms, topography, energy closure or vertical
//! structure. Winds respond to horizontal pressure gradients and are turned
//! by local Coriolis accelerations; the same evolving winds advect moist air
//! and vapor conservatively using spherical finite-volume faces.
//!
//! Aegaea's x direction is WEST and y is NORTH. Solar days are synodic;
//! rotation here is inferred for a *prograde circular orbit* using the
//! already-authored orbital-year duration, never mislabeling a solar day
//! as a sidereal rotation period. Retrograde rotation is not modeled.
use crate::{
    datasets::{MonthlyField, Provenance},
    moisture_transport::{self, Budget, Preview},
    world::{Grid, World},
};
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

pub const WIND_EAST_PREVIEW_ID: &str = "transport.evolving_wind_east";
pub const WIND_NORTH_PREVIEW_ID: &str = "transport.evolving_wind_north";
pub const OPERATOR: &str = "linear_pressure_coriolis_spherical_mass_transport_v1";
const CFL: f64 = 0.5;
const MAX_OUTFLOW_COURANT: f64 = 0.75;
const MAX_SUBSTEPS: usize = 512;
const MAX_WIND_M_S: f64 = 1_000.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicsBudget {
    pub effective_wave_speed_m_s: f64,
    pub angular_velocity_rad_s: f64,
    pub inferred_sidereal_rotation_hours: f64,
    pub max_gravity_wave_courant: f64,
    pub maximum_pressure_acceleration_m_s2: f64,
    pub max_initial_wind_m_s: f64,
    pub max_final_wind_m_s: f64,
    pub assumption: String,
}

#[derive(Debug)]
pub struct DynamicsPreview {
    pub mass: Preview,
    pub wind_east_m_s: Grid,
    pub wind_north_m_s: Grid,
}

#[derive(Clone, Copy)]
struct Face {
    from: usize,
    to: usize,
    sweep_m2_s: f64,
}

fn source<'a>(world: &'a World, kind: MonthlyField) -> Result<&'a Grid, String> {
    let raster = world
        .scientific_fields
        .get(kind.id())
        .ok_or_else(|| format!("Missing {} for selected month", kind.name()))?;
    if raster.width != world.width
        || raster.height != world.height
        || raster.cells.len() != world.width * world.height
        || raster.cells.iter().any(|&v| {
            !v.is_finite() || !(kind.bounds()[0]..=kind.bounds()[1]).contains(&v)
        })
    {
        return Err(format!("Invalid {} raster dimensions or source values", kind.name()));
    }
    Ok(raster)
}

/// Uses the calendar's mean solar day and year length to infer prograde
/// sidereal rotation: Ω = 2π / T_solar + 2π / T_orbit. This is an explicit
/// circular prograde assumption, NOT arbitrary sidereal spin.
fn rotation_rate(world: &World) -> Result<f64, String> {
    world.calendar.validate()?;
    let day = world.calendar.solar_day_hours * 3600.0;
    let year = world.calendar.year_hours() * 3600.0;
    Ok(TAU / day + TAU / year)
}

/// A pure preview: it never mutates an authored dataset, project, or world.
pub fn preview(
    world: &World,
    radius_km: f64,
    gravity_m_s2: f64,
    duration_s: f64,
    effective_wave_speed_m_s: f64,
) -> Result<DynamicsPreview, String> {
    if world.width == 0 || world.height == 0
        || world.width.checked_mul(world.height).is_none()
    {
        return Err("Dynamics require a nonempty finite grid".into());
    }
    if !radius_km.is_finite() || !(100.0..=100_000.0).contains(&radius_km)
        || !gravity_m_s2.is_finite() || !(0.1..=100.0).contains(&gravity_m_s2)
    {
        return Err("Dynamics require valid planet radius and surface gravity".into());
    }
    if !duration_s.is_finite() || !(0.01..=86_400.0).contains(&duration_s) {
        return Err("Dynamics duration must be 0.01–86,400 seconds".into());
    }
    if !effective_wave_speed_m_s.is_finite()
        || !(1.0..=400.0).contains(&effective_wave_speed_m_s)
    {
        return Err("Effective barotropic wave speed must be 1–400 m/s".into());
    }
    let omega = rotation_rate(world)?;
    let pressure = source(world, MonthlyField::Pressure)?;
    let humidity = source(world, MonthlyField::SpecificHumidity)?;
    let east_input = source(world, MonthlyField::WindEast)?;
    let north_input = source(world, MonthlyField::WindNorth)?;

    let (width, height) = (world.width, world.height);
    let n = width * height;
    let radius_m = radius_km * 1000.0;
    let dlon = TAU / width as f64;
    let dlat = PI / height as f64;
    let north_face_m = radius_m * dlat;
    let lat_edge = |j: usize| -FRAC_PI_2 + j as f64 * dlat;
    let mut areas = vec![0.0f64; height];
    let mut lat_sine = vec![0.0f64; height];
    let mut cos_lat = vec![0.0f64; height];
    let mut min_cell_m = f64::INFINITY;
    for y in 0..height {
        areas[y] = radius_m * radius_m * dlon
            * (lat_edge(y + 1).sin() - lat_edge(y).sin());
        let lat = -FRAC_PI_2 + (y as f64 + 0.5) * dlat;
        lat_sine[y] = lat.sin();
        cos_lat[y] = lat.cos();
        if width > 1 {
            min_cell_m = min_cell_m.min(radius_m * cos_lat[y] * dlon);
        }
    }
    if height > 1 {
        min_cell_m = min_cell_m.min(north_face_m);
    }
    if !min_cell_m.is_finite() {
        // One global grid cell has no horizontal neighbors.
        min_cell_m = radius_m;
    }
    let mut air_mass = vec![0.0f64; n];
    let mut vapor_mass = vec![0.0f64; n];
    let mut east: Vec<f64> = east_input.cells.iter().map(|&u| u as f64).collect();
    let mut north: Vec<f64> = north_input.cells.iter().map(|&v| v as f64).collect();
    let mut initial_max_speed = 0.0f64;
    for i in 0..n {
        air_mass[i] = pressure.cells[i] as f64 * 100.0 / gravity_m_s2 * areas[i / width];
        vapor_mass[i] = air_mass[i] * humidity.cells[i] as f64 / 1000.0;
        initial_max_speed = initial_max_speed.max(east[i].hypot(north[i]));
    }
    let air_before: f64 = air_mass.iter().sum();
    let water_before: f64 = vapor_mass.iter().sum();
    let mut air_change = vec![0.0f64; n];
    let mut vapor_change = vec![0.0f64; n];
    let mut accel_e = vec![0.0f64; n];
    let mut accel_n = vec![0.0f64; n];
    let mut faces = Vec::with_capacity(n.saturating_mul(2));
    let mut outgoing = vec![0.0f64; n];

    let mut elapsed = 0.0f64;
    let mut steps = 0usize;
    let mut max_outflow = 0.0f64;
    let mut max_wave_courant = 0.0f64;
    let mut max_accel = 0.0f64;

    while elapsed < duration_s {
        if steps >= MAX_SUBSTEPS {
            return Err("Dynamics need >512 stable substeps. Reduce duration, wind or wave speed.".into());
        }
        // Pressure acceleration of an equivalent barotropic/isothermal
        // column: -c² ∇ln(p). This deliberately omits vertical structure.
        // Use centered geographic distances, one-sided at polar rows.
        for y in 0..height {
            let row = y * width;
            let y0 = y.saturating_sub(1);
            let y1 = (y + 1).min(height - 1);
            for x in 0..width {
                let i = row + x;
                let local_p = air_mass[i] / areas[y]; // proportional to p; g cancels in ∇ln p
                let east_x = (x + width - 1) % width;
                let west_x = (x + 1) % width;
                let east_p = air_mass[row + east_x] / areas[y];
                let west_p = air_mass[row + west_x] / areas[y];
                accel_e[i] = if width > 2 {
                    -effective_wave_speed_m_s.powi(2) * (east_p.ln() - west_p.ln())
                        / (2.0 * radius_m * cos_lat[y] * dlon)
                } else {
                    0.0
                };
                let n_p = air_mass[y1 * width + x] / areas[y1];
                let s_p = air_mass[y0 * width + x] / areas[y0];
                accel_n[i] = if y0 != y1 {
                    -effective_wave_speed_m_s.powi(2) * (n_p.ln() - s_p.ln())
                        / ((y1 - y0) as f64 * north_face_m)
                } else {
                    0.0
                };
                if !local_p.is_finite() || local_p <= 0.0
                    || !accel_e[i].is_finite() || !accel_n[i].is_finite()
                {
                    return Err("Dynamics pressure state became nonfinite or nonpositive".into());
                }
                max_accel = max_accel.max(accel_e[i].hypot(accel_n[i]));
            }
        }

        let max_speed = east.iter().zip(&north)
            .map(|(&u, &v)| u.hypot(v))
            .fold(0.0f64, f64::max);
        if !max_speed.is_finite() || max_speed > MAX_WIND_M_S {
            return Err("Dynamics exceeded wind-speed safety limit".into());
        }
        let remaining = duration_s - elapsed;
        // Acoustic-gravity proxy and material transport must BOTH fit the
        // finest polar cells. A small explicit safety factor also bounds
        // acceleration-generated winds in the same step.
        let dt = remaining.min(CFL * min_cell_m
            / (effective_wave_speed_m_s + max_speed + 1.0));
        if !dt.is_finite() || dt <= 0.0 {
            return Err("No finite dynamics time step available".into());
        }

        // Strang-like half pressure-gradient kick, exact local Coriolis
        // rotation, then half kick. Rotation alone conserves wind speed.
        for y in 0..height {
            let angle = 2.0 * omega * lat_sine[y] * dt;
            let (s, c) = angle.sin_cos();
            for x in 0..width {
                let i = y * width + x;
                let kicked_u = east[i] + 0.5 * dt * accel_e[i];
                let kicked_v = north[i] + 0.5 * dt * accel_n[i];
                east[i] = kicked_u * c + kicked_v * s + 0.5 * dt * accel_e[i];
                north[i] = kicked_v * c - kicked_u * s + 0.5 * dt * accel_n[i];
                if !east[i].is_finite() || !north[i].is_finite()
                    || east[i].hypot(north[i]) > MAX_WIND_M_S
                {
                    return Err("Dynamics produced nonfinite or excessive wind".into());
                }
            }
        }

        faces.clear();
        outgoing.fill(0.0);
        for y in 0..height {
            for x in 0..width {
                let i = y * width + x;
                if width > 1 {
                    let west = y * width + (x + 1) % width;
                    let toward_west = -0.5 * (east[i] + east[west]);
                    if toward_west != 0.0 {
                        let (from, to) = if toward_west > 0.0 {(i, west)} else {(west, i)};
                        let sweep = toward_west.abs() * north_face_m;
                        outgoing[from] += sweep / areas[y];
                        faces.push(Face { from, to, sweep_m2_s: sweep });
                    }
                }
                if y + 1 < height {
                    let neighbor = i + width;
                    let toward_north = 0.5 * (north[i] + north[neighbor]);
                    if toward_north != 0.0 {
                        let (from, to) = if toward_north > 0.0 {(i, neighbor)} else {(neighbor, i)};
                        let sweep = toward_north.abs() * radius_m * dlon
                            * lat_edge(y + 1).cos().max(0.0);
                        outgoing[from] += sweep / areas[from / width];
                        faces.push(Face { from, to, sweep_m2_s: sweep });
                    }
                }
            }
        }
        let courant = outgoing.iter().copied().fold(0.0f64, f64::max) * dt;
        if courant > MAX_OUTFLOW_COURANT + 1.0e-12 {
            return Err(format!("Pressure-driven wind violated outgoing Courant stability ({courant:.3}); shorten duration or reduce wave speed"));
        }
        max_outflow = max_outflow.max(courant);
        max_wave_courant = max_wave_courant.max(effective_wave_speed_m_s * dt / min_cell_m);
        air_change.fill(0.0);
        vapor_change.fill(0.0);
        for face in &faces {
            let fraction = face.sweep_m2_s * dt / areas[face.from / width];
            let moved_air = air_mass[face.from] * fraction;
            let moved_vapor = vapor_mass[face.from] * fraction;
            air_change[face.from] -= moved_air;
            air_change[face.to] += moved_air;
            vapor_change[face.from] -= moved_vapor;
            vapor_change[face.to] += moved_vapor;
        }
        for i in 0..n {
            air_mass[i] += air_change[i];
            vapor_mass[i] += vapor_change[i];
            if !air_mass[i].is_finite() || air_mass[i] <= 0.0
                || !vapor_mass[i].is_finite() || vapor_mass[i] < -1.0e-12 * air_mass[i]
                || vapor_mass[i] > air_mass[i] * (1.0 + 1.0e-12)
            {
                return Err("Dynamics violated positive moist-air/tracer mass".into());
            }
            vapor_mass[i] = vapor_mass[i].max(0.0);
        }
        elapsed += dt;
        steps += 1;
        // At small remaining floating-point intervals, close exactly.
        if duration_s - elapsed <= 1.0e-12 * duration_s {
            elapsed = duration_s;
        }
    }

    let mut vapor_column = Grid::new(width, height, 0.0);
    let mut pressure_map = Grid::new(width, height, 0.0);
    let mut humidity_map = Grid::new(width, height, 0.0);
    let mut east_map = Grid::new(width, height, 0.0);
    let mut north_map = Grid::new(width, height, 0.0);
    let mut final_max_speed = 0.0f64;
    for i in 0..n {
        vapor_column.cells[i] = (vapor_mass[i] / areas[i / width]) as f32;
        pressure_map.cells[i] = (gravity_m_s2 / 100.0
            * air_mass[i] / areas[i / width]) as f32;
        humidity_map.cells[i] = (1000.0 * vapor_mass[i] / air_mass[i]) as f32;
        east_map.cells[i] = east[i] as f32;
        north_map.cells[i] = north[i] as f32;
        final_max_speed = final_max_speed.max(east[i].hypot(north[i]));
    }
    for map in [&vapor_column, &pressure_map, &humidity_map, &east_map, &north_map] {
        if map.cells.iter().any(|v| !v.is_finite()) {
            return Err("Dynamics output exceeds finite map range".into());
        }
    }

    // Verify both conserved substances on the ACTUAL published float32 maps.
    let air_after: f64 = pressure_map.cells.iter().enumerate()
        .map(|(i, &p)| p as f64 * 100.0 / gravity_m_s2 * areas[i / width]).sum();
    let water_after: f64 = vapor_column.cells.iter().enumerate()
        .map(|(i, &w)| w as f64 * areas[i / width]).sum();
    let air_error = (air_after - air_before).abs() / air_before.max(1.0);
    let water_error = (water_after - water_before).abs() / water_before.max(1.0);
    if air_error > 1.0e-6 || water_error > 1.0e-6 {
        return Err(format!("Dynamics mass budget failed: air {air_error:.3e}, vapor {water_error:.3e}"));
    }
    let extrema = |grid: &Grid| -> (f32, f32) {
        (grid.cells.iter().copied().fold(f32::INFINITY, f32::min),
         grid.cells.iter().copied().fold(f32::NEG_INFINITY, f32::max))
    };
    let (min_water, max_water) = extrema(&vapor_column);
    let (min_pressure, max_pressure) = extrema(&pressure_map);
    let (min_humidity, max_humidity) = extrema(&humidity_map);

    let wind_budget = DynamicsBudget {
        effective_wave_speed_m_s,
        angular_velocity_rad_s: omega,
        inferred_sidereal_rotation_hours: TAU / omega / 3600.0,
        max_gravity_wave_courant: max_wave_courant,
        maximum_pressure_acceleration_m_s2: max_accel,
        max_initial_wind_m_s: initial_max_speed,
        max_final_wind_m_s: final_max_speed,
        assumption: "Linearized equivalent barotropic pressure-gradient acceleration -c²∇ln(p), exact local Coriolis rotation f=2Ωsin(latitude), Ω inferred for prograde circular orbit from solar day+year. Cell-centered winds; no nonlinear momentum advection, spherical metric/curvature forces, energy conservation, friction or poles cap model. Conservative moist-air/water continuity only.".into(),
    };
    Ok(DynamicsPreview {
        mass: Preview {
            column_kg_m2: vapor_column,
            pressure_hpa: pressure_map,
            specific_humidity_g_kg: humidity_map,
            budget: Budget {
                month: world.active_month,
                elapsed_seconds: duration_s,
                planet_radius_km: radius_km,
                surface_gravity_m_s2: gravity_m_s2,
                substeps: steps,
                max_outflow_courant: max_outflow,
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
                assumption: "Evolving cell-centered winds under linear pressure-gradient + Coriolis forcing with a user-assumed wave speed, advecting hydrostatic moist air and vapor together. Not a complete shallow-water atmosphere, climate model or precipitation forecast.".into(),
                dynamics: Some(wind_budget),
            },
        },
        wind_east_m_s: east_map,
        wind_north_m_s: north_map,
    })
}

/// Publish all five ephemeral diagnostic maps. This never writes the project
/// or changes authored monthly pressure/humidity/wind rasters.
pub fn publish(world: &mut World, preview: DynamicsPreview) {
    moisture_transport::publish(world, preview.mass);
    let inputs = vec![
        MonthlyField::Pressure.id().into(),
        MonthlyField::SpecificHumidity.id().into(),
        MonthlyField::WindEast.id().into(),
        MonthlyField::WindNorth.id().into(),
        "planet.radius".into(),
        "planet.surface_gravity".into(),
        "planet.calendar".into(),
        "atmosphere.equivalent_wave_speed".into(),
    ];
    for (id, field) in [
        (WIND_EAST_PREVIEW_ID, preview.wind_east_m_s),
        (WIND_NORTH_PREVIEW_ID, preview.wind_north_m_s),
    ] {
        world.scientific_fields.insert(id.into(), field);
        world.scientific_provenance.insert(
            id.into(),
            Provenance::Derived {
                operator: OPERATOR.into(),
                inputs: inputs.clone(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datasets::DatasetRegistry;
    use std::path::PathBuf;

    fn fixture(width: usize, height: usize, p: f32, q: f32, u: f32, v: f32) -> World {
        let mut world = World::empty(PathBuf::from("dynamics-fixture"));
        world.width = width;
        world.height = height;
        let mut sources = DatasetRegistry::default();
        for (kind, val) in [
            (MonthlyField::Pressure, p),
            (MonthlyField::SpecificHumidity, q),
            (MonthlyField::WindEast, u),
            (MonthlyField::WindNorth, v),
        ] {
            sources.create_month(&mut world, kind, 2, val).unwrap();
        }
        world
    }

    #[test]
    fn uniform_stationary_state_does_not_generate_wind_or_water() {
        let world = fixture(24, 12, 1013.25, 12.0, 0.0, 0.0);
        let result = preview(&world, 6371.0, 9.80665, 120.0, 80.0).unwrap();
        assert!(result.wind_east_m_s.cells.iter().all(|&u| u == 0.0));
        assert!(result.wind_north_m_s.cells.iter().all(|&v| v == 0.0));
        assert!(result.mass.pressure_hpa.cells.iter().all(|&p| p == 1013.25));
        assert!(result.mass.budget.air_relative_mass_error < 1.0e-7);
        assert!(result.mass.budget.relative_mass_error < 1.0e-7);
        assert!(result.mass.budget.dynamics.as_ref().unwrap().max_gravity_wave_courant <= CFL);
    }

    #[test]
    fn pressure_gradient_accelerates_toward_low_pressure_across_the_seam() {
        let mut world = fixture(48, 24, 1013.25, 10.0, 0.0, 0.0);
        let p = &mut world.scientific_fields
            .get_mut(MonthlyField::Pressure.id()).unwrap().cells;
        p[12 * 48 + 47] = 900.0; // low east of x=0
        p[12 * 48 + 1] = 1100.0; // high west of x=0
        let result = preview(&world, 6371.0, 9.80665, 180.0, 80.0).unwrap();
        assert!(result.wind_east_m_s.cells[12 * 48] > 0.0,
            "low pressure lies geographically east: eastward wind accelerates");
        assert!(result.mass.budget.air_relative_mass_error < 1.0e-7);
        assert!(result.mass.budget.relative_mass_error < 1.0e-7);
        assert!(result.mass.budget.dynamics.as_ref().unwrap().maximum_pressure_acceleration_m_s2 > 0.0);
    }

    #[test]
    fn coriolis_turns_eastward_wind_south_in_north_and_north_in_south() {
        let world = fixture(24, 24, 1013.25, 10.0, 20.0, 0.0);
        let result = preview(&world, 6371.0, 9.80665, 80.0, 80.0).unwrap();
        assert!(result.wind_north_m_s.cells[18 * 24] < -0.05);
        assert!(result.wind_north_m_s.cells[5 * 24] > 0.05);
        assert!(result.mass.budget.air_relative_mass_error < 1.0e-7);
        assert!(result.mass.budget.relative_mass_error < 1.0e-7);
    }

    #[test]
    fn changing_calendar_day_changes_derived_coriolis_turning_rate() {
        let world = fixture(20, 20, 1013.25, 10.0, 30.0, 0.0);
        let fast = preview(&world, 6371.0, 9.80665, 60.0, 50.0).unwrap();
        let mut slow_world = world;
        slow_world.calendar.solar_day_hours = 48.0;
        let slow = preview(&slow_world, 6371.0, 9.80665, 60.0, 50.0).unwrap();
        let fast_v = fast.wind_north_m_s.cells[15 * 20].abs();
        let slow_v = slow.wind_north_m_s.cells[15 * 20].abs();
        assert!(fast_v > 1.8 * slow_v);
        assert!(fast.mass.budget.dynamics.as_ref().unwrap().angular_velocity_rad_s >
            slow.mass.budget.dynamics.as_ref().unwrap().angular_velocity_rad_s);
    }

    #[test]
    fn evolving_winds_keep_air_and_water_conservative_and_q_bounded() {
        let mut world = fixture(48, 24, 1013.25, 10.0, 30.0, -15.0);
        let p = &mut world.scientific_fields.get_mut(MonthlyField::Pressure.id()).unwrap().cells;
        for (i, item) in p.iter_mut().enumerate() {
            *item = if i % 9 == 0 { 1100.0 } else { 990.0 };
        }
        let q = &mut world.scientific_fields.get_mut(MonthlyField::SpecificHumidity.id()).unwrap().cells;
        for (i, item) in q.iter_mut().enumerate() {
            *item = if i % 7 == 0 { 50.0 } else { 3.0 };
        }
        let pressure_before = p.clone();
        let humidity_before = q.clone();
        let result = preview(&world, 6371.0, 9.80665, 200.0, 60.0).unwrap();
        assert!(result.mass.budget.substeps > 1);
        assert!(result.mass.budget.max_outflow_courant <= MAX_OUTFLOW_COURANT + 1.0e-12);
        assert!(result.mass.budget.air_relative_mass_error < 1.0e-7);
        assert!(result.mass.budget.relative_mass_error < 1.0e-7);
        assert!(result.mass.specific_humidity_g_kg.cells.iter().all(|&q| q >= 0.0 && q <= 50.001));
        assert_eq!(world.scientific_fields[MonthlyField::Pressure.id()].cells, pressure_before);
        assert_eq!(world.scientific_fields[MonthlyField::SpecificHumidity.id()].cells, humidity_before);
    }

    #[test]
    fn rejects_invalid_inputs_and_excessive_steps() {
        let mut world = fixture(64, 32, 1013.25, 10.0, 0.0, 0.0);
        assert!(preview(&world, 6371.0, 9.80665, 0.0, 80.0).is_err());
        assert!(preview(&world, 6371.0, 9.80665, 10.0, f64::NAN).is_err());
        assert!(preview(&world, 6371.0, 0.0, 10.0, 80.0).is_err());
        world.scientific_fields.remove(MonthlyField::WindNorth.id());
        assert!(preview(&world, 6371.0, 9.80665, 10.0, 80.0).is_err());
        let world = fixture(256, 128, 1013.25, 10.0, 0.0, 0.0);
        assert!(preview(&world, 6371.0, 9.80665, 86_400.0, 400.0)
            .unwrap_err().contains("512"));
    }

    #[test]
    fn dynamic_publish_is_ephemeral_and_fixed_preview_clears_winds() {
        let mut world = fixture(24, 12, 1013.25, 10.0, 20.0, 0.0);
        let input = world.scientific_fields[MonthlyField::WindEast.id()].cells.clone();
        let result = preview(&world, 6371.0, 9.80665, 20.0, 50.0).unwrap();
        publish(&mut world, result);
        assert!(world.scientific_fields.contains_key(WIND_EAST_PREVIEW_ID));
        assert!(world.scientific_fields.contains_key(WIND_NORTH_PREVIEW_ID));
        assert_eq!(world.scientific_fields[MonthlyField::WindEast.id()].cells, input);
        assert_eq!(world.scientific_provenance[WIND_EAST_PREVIEW_ID].label(), "derived");
        let fixed = moisture_transport::preview(&world, 6371.0, 9.80665, 20.0).unwrap();
        moisture_transport::publish(&mut world, fixed);
        assert!(!world.scientific_fields.contains_key(WIND_EAST_PREVIEW_ID));
        assert!(world.moisture_transport_budget.as_ref().unwrap().dynamics.is_none());
        assert_eq!(world.scientific_fields[MonthlyField::WindEast.id()].cells, input);
    }
}
