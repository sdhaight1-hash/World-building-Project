//! Instantaneous plate-boundary kinematics from Euler poles, independent of
//! province masks and legacy elevation. This is a *present-day velocity field*,
//! not a plate-history, slab-polarity, or uplift simulation.
use crate::world::{Grid, World};
use std::f64::consts::{PI, TAU};

#[derive(Debug, Clone)]
pub struct KinematicFields {
    /// Positive normal closure at a plate contact (cm/year).
    pub convergence_cm_yr: Grid,
    /// Positive normal opening at a plate contact (cm/year).
    pub divergence_cm_yr: Grid,
    /// Tangential relative motion at a contact (cm/year).
    pub shear_cm_yr: Grid,
}

#[derive(Clone, Copy)]
struct V3(f64, f64, f64);
impl V3 {
    fn add(self, b: Self) -> Self { Self(self.0 + b.0, self.1 + b.1, self.2 + b.2) }
    fn sub(self, b: Self) -> Self { Self(self.0 - b.0, self.1 - b.1, self.2 - b.2) }
    fn scale(self, k: f64) -> Self { Self(self.0 * k, self.1 * k, self.2 * k) }
    fn dot(self, b: Self) -> f64 { self.0 * b.0 + self.1 * b.1 + self.2 * b.2 }
    fn cross(self, b: Self) -> Self {
        Self(self.1 * b.2 - self.2 * b.1,
             self.2 * b.0 - self.0 * b.2,
             self.0 * b.1 - self.1 * b.0)
    }
    fn length(self) -> f64 { self.dot(self).sqrt() }
    fn normalized(self) -> Self { self.scale(1.0 / self.length()) }
}

/// Grid cell-center position in the fictional atlas frame: longitude
/// DECREASES to the right, latitude INCREASES downward.
fn location(x: usize, y: usize, w: usize, h: usize) -> V3 {
    let lon = PI - TAU * (x as f64 + 0.5) / w as f64;
    let lat = -PI / 2.0 + PI * (y as f64 + 0.5) / h as f64;
    V3(lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin())
}

/// 1 km/Ma = 0.1 cm/year. The vector omega has radians per Ma.
fn velocity(omega: V3, location: V3, radius_km: f64) -> V3 {
    omega.cross(location).scale(radius_km * 0.1)
}

/// Compute signed relative motion directly from independently declared Euler
/// poles, not from imported terrain or province boundary classes.
///
/// Returns None only when no plate-motion metadata was supplied (blank-world
/// compatibility). Incomplete/invalid metadata is an error, not silent zeroes.
/// Each contact contributes to BOTH adjacent cells and is weighted by the
/// physical length of the corresponding spherical grid edge.
pub fn derive(world: &World, radius_km: f64) -> Result<Option<KinematicFields>, String> {
    let Some(ids) = world.plate_ids.as_ref() else { return Ok(None) };
    if world.plates.is_empty() { return Ok(None); }
    if !radius_km.is_finite() || radius_km <= 0.0 {
        return Err("Kinematic planet radius must be finite and positive".into());
    }
    let (w, h) = (world.width, world.height);
    if w < 4 || h < 3 || ids.width != w || ids.height != h || ids.cells.len() != w * h {
        return Err("Kinematic plate grid dimensions are inconsistent".into());
    }
    let mut poles: [Option<V3>; 256] = [None; 256];
    for p in &world.plates {
        let (Some(lon), Some(lat), Some(rate)) = (
            p.euler_pole_lon_deg, p.euler_pole_lat_deg, p.signed_rate_deg_per_ma
        ) else {
            return Err(format!("Plate '{}' has no complete Euler pole/rate", p.plate));
        };
        if !lon.is_finite() || !(-180.0..=180.0).contains(&lon)
            || !lat.is_finite() || !(-90.0..=90.0).contains(&lat)
            || !rate.is_finite() || rate.abs() > 20.0
        {
            return Err(format!("Plate '{}' has invalid Euler pole/rate", p.plate));
        }
        let (lo, la) = (lon.to_radians(), lat.to_radians());
        let axis = location_from_angles(lo, la);
        if poles[p.plate_id as usize].replace(axis.scale(rate.to_radians())).is_some() {
            return Err(format!("Repeated plate ID {} in Euler metadata", p.plate_id));
        }
    }
    let mut plate_ids = Vec::with_capacity(w * h);
    for (i, value) in ids.cells.iter().enumerate() {
        if !value.is_finite() || value.fract() != 0.0 || ! (0.0..=255.0).contains(value) {
            return Err(format!("Invalid plate category at cell {i}"));
        }
        let id = *value as usize;
        if poles[id].is_none() {
            return Err(format!("Plate ID {id} has no Euler metadata"));
        }
        plate_ids.push(id);
    }

    let n = w * h;
    let mut closure = vec![0.0f64; n];
    let mut opening = vec![0.0f64; n];
    let mut shear = vec![0.0f64; n];
    let mut contact_length = vec![0.0f64; n];
    let mut add_contact = |a: usize, b: usize, edge_length: f64, pa: V3, pb: V3| {
        if plate_ids[a] == plate_ids[b] { return; }
        let mid = pa.add(pb).normalized();
        // Great-circle tangent pointing from plate A towards plate B.
        let offset = pb.sub(pa);
        let normal = offset.sub(mid.scale(offset.dot(mid))).normalized();
        let v_a = velocity(poles[plate_ids[a]].unwrap(), mid, radius_km);
        let v_b = velocity(poles[plate_ids[b]].unwrap(), mid, radius_km);
        let relative = v_b.sub(v_a);
        let normal_speed = relative.dot(normal);
        let total_speed = relative.length();
        let transverse = (total_speed * total_speed - normal_speed * normal_speed)
            .max(0.0).sqrt();
        // Same operational threshold as the reference dataset: this
        // distinguishes transform-dominated contacts from opening/closure.
        let transform = normal_speed.abs() < (0.28 * total_speed).max(0.9);
        let converge = if transform { 0.0 } else { (-normal_speed).max(0.0) };
        let diverge = if transform { 0.0 } else { normal_speed.max(0.0) };
        let slip = if transform { total_speed } else { transverse };
        for index in [a, b] {
            closure[index] += converge * edge_length;
            opening[index] += diverge * edge_length;
            shear[index] += slip * edge_length;
            contact_length[index] += edge_length;
        }
    };
    for y in 0..h {
        for x in 0..w {
            let a = y * w + x;
            let pa = location(x, y, w, h);
            let right = (x + 1) % w;
            let pb = location(right, y, w, h);
            // Edge separating east/west neighbor cells runs north/south.
            add_contact(a, y * w + right, radius_km * PI / h as f64, pa, pb);
            if y + 1 < h {
                let pc = location(x, y + 1, w, h);
                // Edge between adjacent latitude rows runs east/west.
                let mid_lat = -PI / 2.0 + PI * (y as f64 + 1.0) / h as f64;
                let edge_km = radius_km * TAU / w as f64 * mid_lat.cos().abs();
                add_contact(a, (y + 1) * w + x, edge_km, pa, pc);
            }
        }
    }
    let build = |values: Vec<f64>| Grid {
        width: w,
        height: h,
        cells: values.into_iter().zip(&contact_length).map(|(v, &length)| {
            if length > 0.0 { (v / length) as f32 } else { 0.0 }
        }).collect(),
    };
    Ok(Some(KinematicFields {
        convergence_cm_yr: build(closure),
        divergence_cm_yr: build(opening),
        shear_cm_yr: build(shear),
    }))
}

fn location_from_angles(lon: f64, lat: f64) -> V3 {
    V3(lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::PlateInfo;
    use std::path::PathBuf;

    fn fixture(pole_lon: f64, pole_lat: f64) -> World {
        let mut world = World::empty(PathBuf::from("kinematic-fixture"));
        world.width = 48;
        world.height = 24;
        let mut plates = Grid::new(48, 24, 0.0);
        for y in 0..24 {
            for x in 24..48 { plates.cells[y * 48 + x] = 1.0; }
        }
        world.plate_ids = Some(plates);
        for (id, sign) in [(0, 1.0), (1, -1.0)] {
            world.plates.push(PlateInfo {
                plate_id: id, plate: format!("p{id}"), color_rgb: String::new(),
                euler_pole_lon_deg: Some(pole_lon),
                euler_pole_lat_deg: Some(pole_lat),
                signed_rate_deg_per_ma: Some(sign * 0.4),
            });
        }
        world
    }

    #[test]
    fn east_west_plate_motion_opens_one_margin_and_closes_other() {
        let world = fixture(0.0, 90.0);
        let f = derive(&world, 6371.0).unwrap().unwrap();
        assert!(f.divergence_cm_yr.at(23, 12) > 1.0);
        assert!(f.convergence_cm_yr.at(47, 12) > 1.0);
        // Both sides receive the same contact signal.
        assert_eq!(f.divergence_cm_yr.at(23, 12), f.divergence_cm_yr.at(24, 12));
        assert!(f.convergence_cm_yr.cells.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn reversing_rotations_reverses_opening_and_closure() {
        let mut world = fixture(0.0, 90.0);
        let before = derive(&world, 6371.0).unwrap().unwrap();
        for plate in &mut world.plates {
            *plate.signed_rate_deg_per_ma.as_mut().unwrap() *= -1.0;
        }
        let after = derive(&world, 6371.0).unwrap().unwrap();
        assert!((before.divergence_cm_yr.at(23, 12)
            - after.convergence_cm_yr.at(23, 12)).abs() < 0.001);
    }

    #[test]
    fn strike_slip_is_not_misclassified_as_mountain_convergence() {
        let world = fixture(90.0, 0.0);
        let f = derive(&world, 6371.0).unwrap().unwrap();
        assert!(f.shear_cm_yr.at(23, 12) > 1.0);
        assert_eq!(f.convergence_cm_yr.at(23, 12), 0.0);
        assert_eq!(f.divergence_cm_yr.at(23, 12), 0.0);
    }

    #[test]
    fn no_metadata_remains_blank_world_compatible() {
        let mut world = fixture(0.0, 90.0);
        world.plates.clear();
        assert!(derive(&world, 6371.0).unwrap().is_none());
    }

    #[test]
    fn incomplete_motion_is_explicit_error() {
        let mut world = fixture(0.0, 90.0);
        world.plates[0].signed_rate_deg_per_ma = None;
        assert!(derive(&world, 6371.0).is_err());
    }

    #[test]
    fn actual_aegaea_plate_motion_is_finite_and_has_all_three_classes() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../data/aegaea/tectonics/v1");
        let mut world = World::load(root).unwrap();
        world.elevation = None;
        let f = derive(&world, 6371.0).unwrap().unwrap();
        for grid in [&f.convergence_cm_yr, &f.divergence_cm_yr, &f.shear_cm_yr] {
            assert!(grid.cells.iter().all(|v| v.is_finite() && *v >= 0.0));
            assert!(grid.cells.iter().any(|v| *v > 0.0));
        }
    }
}
