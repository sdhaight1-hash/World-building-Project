//! Opt-in spherical barotropic-column momentum transport, separate from the
//! historical linearized dynamics preview.
//!
//! Finite-volume conserved quantities: moist-air mass, vapor mass and
//! **absolute axial angular momentum** (AAM). The last is the genuine global
//! momentum invariant on an axisymmetric, unforced rotating sphere.
//! Northward momentum is also advected conservatively through shared upwind
//! mass fluxes, with explicit spherical curvature/Coriolis, meridional
//! pressure forces and geometric pressure balancing.
//!
//! Pressure is P = c² * column mass density, in N/m (barotropic isothermal
//! sheet). It is not a 3-D atmosphere or a conventional free-surface shallow
//! water model. First-order donor-cell fluxes dissipate energy; report their
//! kinetic + compressional energy drift, do not claim energy conservation.
//! Polar boundaries are the exactly zero-length edges of spherical caps;
//! near-pole cell width controls the timestep, without artificial cos limits.
//! This is NOT pole-crossing dynamics or a validated global climate model.
use crate::{
    atmosphere_dynamics::{self, DynamicsBudget, DynamicsPreview},
    datasets::MonthlyField,
    moisture_transport::{Budget, Preview},
    world::{Grid, World},
};
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

pub const OPERATOR: &str = "spherical_barotropic_absolute_axial_momentum_fv_v1";
const CFL: f64 = 0.40;
const OUTFLOW_CFL: f64 = 0.70;
const MAX_SUBSTEPS: usize = 512;
const MAX_WIND_M_S: f64 = 1000.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MomentumAudit {
    pub absolute_axial_momentum_before_kg_m2_s: f64,
    pub absolute_axial_momentum_after_kg_m2_s: f64,
    pub relative_axial_momentum_error: f64,
    pub kinetic_energy_j_before: f64,
    pub kinetic_energy_j_after: f64,
    pub compressional_energy_j_before: f64,
    pub compressional_energy_j_after: f64,
    pub total_energy_j_before: f64,
    pub total_energy_j_after: f64,
    pub relative_energy_drift: f64,
    pub maximum_outflow_courant: f64,
    pub minimum_polar_cell_width_m: f64,
    pub pole_boundary_mass_flux_kg: f64,
    pub note: String,
}

fn source<'a>(world: &'a World, kind: MonthlyField) -> Result<&'a Grid, String> {
    let g = world.scientific_fields.get(kind.id())
        .ok_or_else(|| format!("Missing monthly {}", kind.name()))?;
    if g.width != world.width || g.height != world.height
        || g.cells.len() != world.width * world.height
        || g.cells.iter().any(|&v| {
            !v.is_finite() || !(kind.bounds()[0]..=kind.bounds()[1]).contains(&v)
        })
    {
        return Err(format!("Invalid dimensions or input values: {}", kind.name()));
    }
    Ok(g)
}

fn extrema(g: &Grid) -> (f32, f32) {
    (
        g.cells.iter().copied().fold(f32::INFINITY, f32::min),
        g.cells.iter().copied().fold(f32::NEG_INFINITY, f32::max),
    )
}

fn energy(mass: &[f64], axial: &[f64], north_m: &[f64],
          areas: &[f64], cos_lat: &[f64], width: usize, radius: f64,
          omega: f64, c2: f64, reference_column: f64) -> (f64, f64) {
    let mut kinetic = 0.0;
    let mut compressional = 0.0;
    for i in 0..mass.len() {
        let y = i / width;
        let arm = radius * cos_lat[y];
        let u = axial[i] / mass[i] / arm - omega * arm;
        let v = north_m[i] / mass[i];
        kinetic += 0.5 * mass[i] * (u * u + v * v);
        let delta = mass[i] / (areas[y] * reference_column) - 1.0;
        // Positive relative isothermal potential energy. Subtracting the
        // mass-constant linear term makes a resting uniform column numerically
        // zero instead of subtracting enormous nearly equal global energies.
        let entropy = if delta.abs() < 1.0e-4 {
            let d2 = delta * delta;
            d2 * (0.5 - delta / 6.0 + d2 / 12.0)
        } else {
            (1.0 + delta) * delta.ln_1p() - delta
        };
        compressional += c2 * reference_column * areas[y] * entropy;
    }
    (kinetic, compressional)
}

/// Pure, short-interval, mass- and axial-angular-momentum-conservative preview.
/// Pressure does horizontal work; kinetic/column-energy drift is audited but
/// first-order upwinding is dissipative and not an energy-preserving discretization.
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
        return Err("Spherical momentum requires nonempty grid dimensions".into());
    }
    if !radius_km.is_finite() || !(100.0..=100_000.0).contains(&radius_km)
        || !gravity_m_s2.is_finite() || !(0.1..=100.0).contains(&gravity_m_s2)
    {
        return Err("Spherical momentum requires valid radius and gravity".into());
    }
    if !duration_s.is_finite() || !(0.01..=86_400.0).contains(&duration_s)
        || !effective_wave_speed_m_s.is_finite()
        || !(1.0..=400.0).contains(&effective_wave_speed_m_s)
    {
        return Err("Invalid momentum interval or effective wave speed (1–400 m/s)".into());
    }
    let omega = atmosphere_dynamics::rotation_rate(world)?;
    let pressure = source(world, MonthlyField::Pressure)?;
    let humidity = source(world, MonthlyField::SpecificHumidity)?;
    let authored_u = source(world, MonthlyField::WindEast)?;
    let authored_v = source(world, MonthlyField::WindNorth)?;

    let (width, height) = (world.width, world.height);
    let n = width * height;
    let a = radius_km * 1000.0;
    let dl = TAU / width as f64;
    let dp = PI / height as f64;
    let d_meridian = a * dp;
    let edge_lat = |y: usize| -FRAC_PI_2 + y as f64 * dp;
    let mut area = vec![0.0f64; height];
    let mut cos_lat = vec![0.0f64; height];
    let mut tan_lat = vec![0.0f64; height];
    let mut coriolis = vec![0.0f64; height];
    let mut north_edges = vec![0.0f64; height + 1];
    let mut min_width = f64::INFINITY;
    for (y, edge) in north_edges.iter_mut().enumerate() {
        *edge = a * edge_lat(y).cos().max(0.0) * dl;
    }
    // An actual pole is a point: no artificial flux across north/south ends.
    north_edges[0] = 0.0;
    north_edges[height] = 0.0;
    for y in 0..height {
        area[y] = a * a * dl * (edge_lat(y + 1).sin() - edge_lat(y).sin());
        let phi = -FRAC_PI_2 + (y as f64 + 0.5) * dp;
        cos_lat[y] = phi.cos();
        tan_lat[y] = phi.tan();
        coriolis[y] = 2.0 * omega * phi.sin();
        if width > 1 {
            min_width = min_width.min(a * cos_lat[y] * dl);
        }
    }
    if height > 1 {
        min_width = min_width.min(d_meridian);
    }
    if !min_width.is_finite() {
        min_width = a;
    }
    // Fail before allocation/work for impossible high-resolution polar CFL.
    let lower_steps = (duration_s * (effective_wave_speed_m_s + 1.0)
        / (CFL * min_width)).ceil();
    if !lower_steps.is_finite() || lower_steps > MAX_SUBSTEPS as f64 {
        return Err(format!(
            "Spherical polar CFL requires at least {lower_steps:.0} substeps (limit 512); shorten interval or reduce wave speed."
        ));
    }

    let mut mass = vec![0.0f64; n];
    let mut vapor = vec![0.0f64; n];
    let mut aam = vec![0.0f64; n];
    let mut meridional = vec![0.0f64; n];
    let mut max_initial_wind = 0.0f64;
    let mut area_sum = 0.0;
    for y in 0..height {
        area_sum += width as f64 * area[y];
        let arm = a * cos_lat[y];
        for x in 0..width {
            let i = y * width + x;
            mass[i] = pressure.cells[i] as f64 * 100.0 / gravity_m_s2 * area[y];
            vapor[i] = mass[i] * humidity.cells[i] as f64 / 1000.0;
            let u = authored_u.cells[i] as f64;
            let v = authored_v.cells[i] as f64;
            aam[i] = mass[i] * arm * (u + omega * arm);
            meridional[i] = mass[i] * v;
            max_initial_wind = max_initial_wind.max(u.hypot(v));
        }
    }
    let mass_before: f64 = mass.iter().sum();
    let vapor_before: f64 = vapor.iter().sum();
    let aam_before: f64 = aam.iter().sum();
    let reference_column = mass_before / area_sum;
    let c2 = effective_wave_speed_m_s * effective_wave_speed_m_s;
    let (ke_before, ce_before) = energy(
        &mass, &aam, &meridional, &area, &cos_lat, width, a, omega, c2, reference_column
    );

    let mut dm = vec![0.0; n];
    let mut dw = vec![0.0; n];
    let mut dlz = vec![0.0; n];
    let mut dq = vec![0.0; n];
    let mut outgoing = vec![0.0; n];
    let mut u = vec![0.0; n];
    let mut v = vec![0.0; n];
    let mut steps = 0usize;
    let mut elapsed = 0.0;
    let mut max_courant = 0.0f64;
    let mut max_wave_courant = 0.0f64;
    let mut max_pressure_accel = 0.0f64;

    while elapsed < duration_s {
        if steps >= MAX_SUBSTEPS {
            return Err("Spherical momentum needs >512 stable steps; shorten interval".into());
        }
        let mut speed = 0.0f64;
        for i in 0..n {
            let y = i / width;
            let arm = a * cos_lat[y];
            u[i] = aam[i] / mass[i] / arm - omega * arm;
            v[i] = meridional[i] / mass[i];
            speed = speed.max(u[i].hypot(v[i]));
        }
        if !speed.is_finite() || speed > MAX_WIND_M_S {
            return Err("Spherical momentum exceeded safe wind-speed bounds".into());
        }
        let remaining = duration_s - elapsed;
        let dt_wave = CFL * min_width / (effective_wave_speed_m_s + speed + 1.0);
        let mut dt = remaining.min(dt_wave);
        if !dt.is_finite() || dt <= 0.0 {
            return Err("No positive finite momentum timestep".into());
        }

        outgoing.fill(0.0);
        // Compute a donor outflow bound including both longitude and
        // latitude faces, in addition to the barotropic gravity-wave limit.
        for y in 0..height {
            for x in 0..width {
                let i = y * width + x;
                if width > 1 {
                    let j = y * width + (x + 1) % width;
                    let west = -0.5 * (u[i] + u[j]);
                    let from = if west > 0.0 { i } else { j };
                    outgoing[from] += west.abs() * d_meridian / area[y];
                }
                if y + 1 < height {
                    let j = i + width;
                    let north = 0.5 * (v[i] + v[j]);
                    let from = if north > 0.0 { i } else { j };
                    outgoing[from] += north.abs() * north_edges[y + 1] / area[from / width];
                }
            }
        }
        let fastest = outgoing.iter().copied().fold(0.0f64, f64::max);
        if fastest > 0.0 {
            dt = dt.min(OUTFLOW_CFL / fastest);
        }
        if !dt.is_finite() || dt <= 0.0 {
            return Err("No positive CFL-safe momentum timestep".into());
        }
        max_courant = max_courant.max(dt * fastest);
        max_wave_courant = max_wave_courant.max(effective_wave_speed_m_s * dt / min_width);
        dm.fill(0.0);
        dw.fill(0.0);
        dlz.fill(0.0);
        dq.fill(0.0);

        // Conservative upwind face exchange for mass, vapor, angular
        // momentum and northward momentum. Pressure torques at each
        // meridian face are antisymmetric, so total axial AM is exact.
        for y in 0..height {
            let torque_edge = a * a * (edge_lat(y + 1).sin() - edge_lat(y).sin());
            for x in 0..width {
                let i = y * width + x;
                if width > 1 {
                    let j = y * width + (x + 1) % width; // geographically west
                    let west = -0.5 * (u[i] + u[j]);
                    if west != 0.0 {
                        let (from, to) = if west > 0.0 { (i, j) } else { (j, i) };
                        let share = west.abs() * d_meridian * dt / area[y];
                        let air = mass[from] * share;
                        let water = vapor[from] * share;
                        let torque = aam[from] * share;
                        let north_m = meridional[from] * share;
                        dm[from] -= air; dm[to] += air;
                        dw[from] -= water; dw[to] += water;
                        dlz[from] -= torque; dlz[to] += torque;
                        dq[from] -= north_m; dq[to] += north_m;
                    }
                    let h_face = 0.5 * (mass[i] + mass[j]) / area[y];
                    let p_force = c2 * h_face;
                    let torque = dt * p_force * torque_edge;
                    dlz[i] += torque;
                    dlz[j] -= torque;
                    max_pressure_accel = max_pressure_accel.max(
                        (c2 * ((mass[i] / area[y]).ln() - (mass[j] / area[y]).ln())
                            / (a * cos_lat[y] * dl)).abs()
                    );
                }
                if y + 1 < height {
                    let j = i + width;
                    let north = 0.5 * (v[i] + v[j]);
                    if north != 0.0 {
                        let (from, to) = if north > 0.0 { (i, j) } else { (j, i) };
                        let share = north.abs() * north_edges[y + 1] * dt
                            / area[from / width];
                        let air = mass[from] * share;
                        let water = vapor[from] * share;
                        let torque = aam[from] * share;
                        let north_m = meridional[from] * share;
                        dm[from] -= air; dm[to] += air;
                        dw[from] -= water; dw[to] += water;
                        dlz[from] -= torque; dlz[to] += torque;
                        dq[from] -= north_m; dq[to] += north_m;
                    }
                    let h_face = 0.5 * (
                        mass[i] / area[y] + mass[j] / area[y + 1]
                    );
                    let pressure_force = dt * c2 * h_face * north_edges[y + 1];
                    dq[i] -= pressure_force;
                    dq[j] += pressure_force;
                }
            }
        }

        for y in 0..height {
            let geometry = north_edges[y + 1] - north_edges[y];
            for x in 0..width {
                let i = y * width + x;
                let h = mass[i] / area[y];
                // Pressure face-length geometry cancels exactly for a
                // uniform hydrostatic column, including the polar caps.
                dq[i] += dt * c2 * h * geometry;
                // Curvature + Coriolis:
                // Dv/Dt = -(2 Ω sinφ + u tanφ/a) u - (∇p)/ρ.
                dq[i] -= dt * mass[i] *
                    (coriolis[y] * u[i] + u[i] * u[i] * tan_lat[y] / a);
                let new_mass = mass[i] + dm[i];
                let new_vapor = vapor[i] + dw[i];
                if !new_mass.is_finite() || new_mass <= 0.0
                    || !new_vapor.is_finite()
                    || new_vapor < -1.0e-12 * new_mass
                    || new_vapor > new_mass * (1.0 + 1.0e-12)
                {
                    return Err("Spherical momentum violated air/vapor positivity".into());
                }
                mass[i] = new_mass;
                vapor[i] = new_vapor.max(0.0);
                aam[i] += dlz[i];
                meridional[i] += dq[i];
                if !aam[i].is_finite() || !meridional[i].is_finite() {
                    return Err("Spherical momentum became nonfinite".into());
                }
            }
        }
        elapsed += dt;
        steps += 1;
        if duration_s - elapsed <= duration_s * 1.0e-12 {
            elapsed = duration_s;
        }
    }

    let mut vapor_field = Grid::new(width, height, 0.0);
    let mut pressure_field = Grid::new(width, height, 0.0);
    let mut humidity_field = Grid::new(width, height, 0.0);
    let mut east_field = Grid::new(width, height, 0.0);
    let mut north_field = Grid::new(width, height, 0.0);
    let mut max_final_wind = 0.0f64;
    for i in 0..n {
        let y = i / width;
        let arm = a * cos_lat[y];
        let final_u = aam[i] / mass[i] / arm - omega * arm;
        let final_v = meridional[i] / mass[i];
        max_final_wind = max_final_wind.max(final_u.hypot(final_v));
        vapor_field.cells[i] = (vapor[i] / area[y]) as f32;
        pressure_field.cells[i] = (gravity_m_s2 * mass[i] / area[y] / 100.0) as f32;
        humidity_field.cells[i] = (1000.0 * vapor[i] / mass[i]) as f32;
        east_field.cells[i] = final_u as f32;
        north_field.cells[i] = final_v as f32;
    }
    for g in [&vapor_field, &pressure_field, &humidity_field, &east_field, &north_field] {
        if g.cells.iter().any(|&v| !v.is_finite()) {
            return Err("Spherical momentum output exceeded map numeric range".into());
        }
    }
    let mass_after: f64 = pressure_field.cells.iter().enumerate()
        .map(|(i, &p)| p as f64 * 100.0 / gravity_m_s2 * area[i / width]).sum();
    let vapor_after: f64 = vapor_field.cells.iter().enumerate()
        .map(|(i, &w)| w as f64 * area[i / width]).sum();
    let aam_after: f64 = pressure_field.cells.iter().zip(&east_field.cells)
        .enumerate().map(|(i, (&p, &u))| {
            let y = i / width;
            let arm = a * cos_lat[y];
            p as f64 * 100.0 / gravity_m_s2 * area[y]
                * arm * (u as f64 + omega * arm)
        }).sum();
    let air_error = (mass_after - mass_before).abs() / mass_before.max(1.0);
    let water_error = (vapor_after - vapor_before).abs() / vapor_before.max(1.0);
    let aam_error = (aam_after - aam_before).abs() / aam_before.abs().max(1.0);
    if air_error > 1.0e-6 || water_error > 1.0e-6 || aam_error > 1.0e-6 {
        return Err(format!(
            "Spherical momentum conservation failed: air {air_error:.3e}, water {water_error:.3e}, axial AM {aam_error:.3e}"
        ));
    }
    let (ke_after, ce_after) = energy(
        &mass, &aam, &meridional, &area, &cos_lat, width, a, omega, c2, reference_column
    );
    let energy_before = ke_before + ce_before;
    let energy_after = ke_after + ce_after;
    let energy_drift = (energy_after - energy_before) / energy_before.abs().max(1.0);
    if !energy_before.is_finite() || !energy_after.is_finite() {
        return Err("Spherical momentum has invalid energy diagnostics".into());
    }
    let (min_water, max_water) = extrema(&vapor_field);
    let (min_pressure, max_pressure) = extrema(&pressure_field);
    let (min_q, max_q) = extrema(&humidity_field);
    let audit = MomentumAudit {
        absolute_axial_momentum_before_kg_m2_s: aam_before,
        absolute_axial_momentum_after_kg_m2_s: aam_after,
        relative_axial_momentum_error: aam_error,
        kinetic_energy_j_before: ke_before,
        kinetic_energy_j_after: ke_after,
        compressional_energy_j_before: ce_before,
        compressional_energy_j_after: ce_after,
        total_energy_j_before: energy_before,
        total_energy_j_after: energy_after,
        relative_energy_drift: energy_drift,
        maximum_outflow_courant: max_courant,
        minimum_polar_cell_width_m: min_width,
        pole_boundary_mass_flux_kg: 0.0,
        note: "Global absolute axial AM uses total column mass times a*cos(phi)*(u+Omega*a*cos(phi)); computed on exported f32 pressure/wind. First-order upwind dissipates kinetic/compressional energy, which is DIAGNOSED not numerically enforced. Spherical polar caps have vanishing north/south face lengths; physical pole-crossing and angular-momentum/energy benchmark tests remain future work.".into(),
    };
    let dynamics = DynamicsBudget {
        effective_wave_speed_m_s,
        angular_velocity_rad_s: omega,
        inferred_sidereal_rotation_hours: TAU / omega / 3600.0,
        max_gravity_wave_courant: max_wave_courant,
        maximum_pressure_acceleration_m_s2: max_pressure_accel,
        max_initial_wind_m_s: max_initial_wind,
        max_final_wind_m_s: max_final_wind,
        assumption: "Conservative spherical moist-air, vapor and absolute axial angular momentum with paired barotropic pressure torques; meridional momentum donor-cell flux plus full spherical curvature/Coriolis and geometric pressure balancing. No vertical structure, full vector momentum conservation, energy-preserving flux, true pole-crossing or precipitation.".into(),
        momentum: Some(audit),
    };
    Ok(DynamicsPreview {
        mass: Preview {
            column_kg_m2: vapor_field,
            pressure_hpa: pressure_field,
            specific_humidity_g_kg: humidity_field,
            budget: Budget {
                month: world.active_month,
                elapsed_seconds: duration_s,
                planet_radius_km: radius_km,
                surface_gravity_m_s2: gravity_m_s2,
                substeps: steps,
                max_outflow_courant: max_courant,
                air_kg_before: mass_before,
                air_kg_after: mass_after,
                air_relative_mass_error: air_error,
                water_kg_before: vapor_before,
                water_kg_after: vapor_after,
                relative_mass_error: water_error,
                minimum_pressure_hpa: min_pressure,
                maximum_pressure_hpa: max_pressure,
                minimum_humidity_g_kg: min_q,
                maximum_humidity_g_kg: max_q,
                minimum_column_kg_m2: min_water,
                maximum_column_kg_m2: max_water,
                operator: OPERATOR.into(),
                assumption: "Axisymmetric spherical barotropic column; actual polar-cap geometry, conserved axial angular momentum and two conserved mass tracers. First-order dissipative fluxes; no nonlinear vertical physics, full primitive-equation dynamics or rainfall.".into(),
                dynamics: Some(dynamics),
            },
        },
        wind_east_m_s: east_field,
        wind_north_m_s: north_field,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datasets::DatasetRegistry;
    use std::path::PathBuf;

    fn fixture(w: usize, h: usize, p: f32, q: f32, u: f32, v: f32) -> World {
        let mut world = World::empty(PathBuf::from("spherical-momentum-fixture"));
        world.width = w;
        world.height = h;
        let mut registry = DatasetRegistry::default();
        for (kind, value) in [
            (MonthlyField::Pressure, p),
            (MonthlyField::SpecificHumidity, q),
            (MonthlyField::WindEast, u),
            (MonthlyField::WindNorth, v),
        ] {
            registry.create_month(&mut world, kind, 3, value).unwrap();
        }
        world
    }

    #[test]
    fn hydrostatic_rest_is_well_balanced_even_in_polar_caps() {
        let w = fixture(48, 24, 1013.25, 10.0, 0.0, 0.0);
        let r = preview(&w, 6371.0, 9.80665, 60.0, 80.0).unwrap();
        assert!(r.mass.pressure_hpa.cells.iter().all(|&p| (p - 1013.25).abs() < 0.001));
        assert!(r.wind_east_m_s.cells.iter().all(|&u| u.abs() < 1.0e-5));
        assert!(r.wind_north_m_s.cells.iter().all(|&v| v.abs() < 1.0e-5));
        let am = r.mass.budget.dynamics.as_ref().unwrap().momentum.as_ref().unwrap();
        assert!(am.relative_axial_momentum_error < 1.0e-7);
        assert!(am.relative_energy_drift.abs() < 1.0e-6);
        assert_eq!(am.pole_boundary_mass_flux_kg, 0.0);
    }

    #[test]
    fn zonal_pressure_torques_move_east_across_seam_without_global_axial_gain() {
        let mut w = fixture(48, 24, 1013.25, 10.0, 0.0, 0.0);
        let p = &mut w.scientific_fields.get_mut(MonthlyField::Pressure.id()).unwrap().cells;
        p[12 * 48 + 47] = 900.0;
        p[12 * 48 + 1] = 1100.0;
        let r = preview(&w, 6371.0, 9.80665, 180.0, 80.0).unwrap();
        assert!(r.wind_east_m_s.cells[12 * 48] > 0.0);
        assert!(r.mass.budget.dynamics.as_ref().unwrap().momentum.as_ref().unwrap()
            .relative_axial_momentum_error < 1.0e-7);
        assert!(r.mass.budget.air_relative_mass_error < 1.0e-7);
    }

    #[test]
    fn meridional_advection_generates_correct_coriolis_and_curvature() {
        let w = fixture(40, 20, 1013.25, 10.0, 0.0, 30.0);
        let r = preview(&w, 6371.0, 9.80665, 120.0, 60.0).unwrap();
        assert!(r.wind_east_m_s.cells[15 * 40] > 0.0);
        assert!(r.wind_east_m_s.cells[4 * 40] < 0.0);
        let m = r.mass.budget.dynamics.as_ref().unwrap().momentum.as_ref().unwrap();
        assert!(m.relative_axial_momentum_error < 1.0e-7);
        assert!(r.mass.budget.relative_mass_error < 1.0e-7);
    }

    #[test]
    fn planetary_rotation_and_gravity_change_dynamics_without_changing_sources() {
        let mut w = fixture(36, 18, 1013.25, 10.0, 20.0, 15.0);
        let source = w.scientific_fields[MonthlyField::WindEast.id()].cells.clone();
        let fast = preview(&w, 6371.0, 9.80665, 80.0, 50.0).unwrap();
        w.calendar.solar_day_hours = 48.0;
        let slow = preview(&w, 6371.0, 3.71, 80.0, 50.0).unwrap();
        let a = fast.mass.budget.dynamics.unwrap();
        let b = slow.mass.budget.dynamics.unwrap();
        assert!(a.angular_velocity_rad_s > b.angular_velocity_rad_s);
        assert_eq!(w.scientific_fields[MonthlyField::WindEast.id()].cells, source);
        assert!(slow.mass.budget.air_relative_mass_error < 1.0e-7);
    }

    #[test]
    fn chaotic_moisture_redistribution_preserves_mass_and_axial_momentum() {
        let mut w = fixture(64, 32, 1013.25, 10.0, 20.0, -12.0);
        for (i, p) in w.scientific_fields.get_mut(MonthlyField::Pressure.id())
            .unwrap().cells.iter_mut().enumerate() {
            *p = if i % 11 == 0 { 1100.0 } else { 995.0 };
        }
        for (i, q) in w.scientific_fields.get_mut(MonthlyField::SpecificHumidity.id())
            .unwrap().cells.iter_mut().enumerate() {
            *q = if i % 7 == 0 { 45.0 } else { 3.0 };
        }
        let original = w.scientific_fields[MonthlyField::Pressure.id()].cells.clone();
        let r = preview(&w, 6371.0, 9.80665, 800.0, 60.0).unwrap();
        let budget = &r.mass.budget;
        assert!(budget.substeps > 1);
        assert!(budget.air_relative_mass_error < 1.0e-7);
        assert!(budget.relative_mass_error < 1.0e-7);
        let am = budget.dynamics.as_ref().unwrap().momentum.as_ref().unwrap();
        assert!(am.relative_axial_momentum_error < 1.0e-7);
        assert!(am.total_energy_j_after.is_finite());
        assert!(am.relative_energy_drift.is_finite());
        assert!(r.mass.specific_humidity_g_kg.cells.iter()
            .all(|&q| q >= 0.0 && q <= 45.001));
        assert_eq!(w.scientific_fields[MonthlyField::Pressure.id()].cells, original);
    }

    #[test]
    fn high_resolution_polar_cfl_fails_early() {
        let w = fixture(256, 128, 1013.25, 10.0, 0.0, 0.0);
        let result = preview(&w, 6371.0, 9.80665, 86_400.0, 400.0).unwrap_err();
        assert!(result.contains("polar CFL"));
    }

    #[test]
    fn invalid_assumptions_are_rejected() {
        let w = fixture(24, 12, 1013.25, 10.0, 0.0, 0.0);
        assert!(preview(&w, f64::NAN, 9.80665, 20.0, 80.0).is_err());
        assert!(preview(&w, 6371.0, 0.0, 20.0, 80.0).is_err());
        assert!(preview(&w, 6371.0, 9.80665, 20.0, f64::NAN).is_err());
        assert!(preview(&w, 6371.0, 9.80665, 0.0, 80.0).is_err());
    }
}
