//! Thematic coloring and a first, projection-correct atlas / globe preview.
//! These CPU-projected images are uploaded as egui textures to the wgpu backend.
//! Production geometry and shader-based rendering are planned separately.
use crate::world::{geo_to_uv, uv_to_geo, World};
use eframe::egui::{self, Color32};
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Terrain,
    Plates,
    Cratons,
    Provinces,
    Crust,
    Lithosphere,
    HeatFlow,
}

impl Theme {
    pub const ALL: [Self; 7] = [
        Self::Terrain,
        Self::Plates,
        Self::Cratons,
        Self::Provinces,
        Self::Crust,
        Self::Lithosphere,
        Self::HeatFlow,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Terrain => "Physical terrain",
            Self::Plates => "Tectonic plates",
            Self::Cratons => "Cratonic basement",
            Self::Provinces => "Crustal provinces",
            Self::Crust => "Crust thickness",
            Self::Lithosphere => "Lithosphere depth",
            Self::HeatFlow => "Surface heat flow",
        }
    }

    pub fn available(self, w: &World) -> bool {
        match self {
            Self::Terrain => w.elevation.is_some(),
            Self::Plates => w.plate_ids.is_some(),
            Self::Cratons => w.craton_ids.is_some(),
            Self::Provinces => w.province_ids.is_some(),
            Self::Crust => w.crust_km.is_some(),
            Self::Lithosphere => w.lithosphere_km.is_some(),
            Self::HeatFlow => w.heatflow.is_some(),
        }
    }

    pub fn units(self) -> &'static str {
        match self {
            Self::Terrain => "m",
            Self::Crust | Self::Lithosphere => "km",
            Self::HeatFlow => "mW/m²",
            _ => "category",
        }
    }
}

fn mix(a: [u8; 3], b: [u8; 3], amount: f32) -> [u8; 3] {
    let amount = amount.clamp(0.0, 1.0);
    let ch = |i| ((a[i] as f32) * (1.0 - amount) + (b[i] as f32) * amount).round() as u8;
    [ch(0), ch(1), ch(2)]
}

fn ramp(value: f32, lo: f32, hi: f32) -> [u8; 3] {
    if !value.is_finite() {
        return [77, 82, 96];
    }
    let t = ((value - lo) / (hi - lo)).clamp(0.0, 1.0);
    let cold = [32, 93, 150];
    let mid = [83, 201, 178];
    let hot = [235, 178, 90];
    if t < 0.5 {
        mix(cold, mid, t * 2.0)
    } else {
        mix(mid, hot, (t - 0.5) * 2.0)
    }
}

fn elevation_color(m: f32) -> [u8; 3] {
    if !m.is_finite() {
        return [45, 50, 59];
    }
    if m < 0.0 {
        let t = ((m + 9000.0) / 9000.0).clamp(0.0, 1.0);
        return mix([8, 32, 67], [39, 123, 174], t);
    }
    if m < 650.0 {
        mix([50, 111, 81], [112, 156, 91], m / 650.0)
    } else if m < 2000.0 {
        mix([112, 156, 91], [176, 150, 99], (m - 650.0) / 1350.0)
    } else if m < 4300.0 {
        mix([176, 150, 99], [117, 104, 98], (m - 2000.0) / 2300.0)
    } else {
        mix([117, 104, 98], [234, 239, 238], (m - 4300.0) / 2500.0)
    }
}

const PALETTE: [[u8; 3]; 18] = [
    [103, 157, 177],
    [223, 157, 102],
    [68, 169, 148],
    [203, 122, 96],
    [164, 123, 205],
    [61, 144, 198],
    [227, 185, 107],
    [126, 162, 101],
    [75, 161, 185],
    [172, 127, 199],
    [190, 113, 139],
    [194, 182, 94],
    [104, 135, 191],
    [113, 176, 131],
    [203, 145, 168],
    [119, 180, 184],
    [180, 153, 126],
    [160, 157, 190],
];

pub fn sample_rgb(world: &World, theme: Theme, x: usize, y: usize) -> [u8; 3] {
    let base = world
        .elevation
        .as_ref()
        .map(|grid| elevation_color(grid.at(x, y)))
        .unwrap_or([35, 49, 69]);
    match theme {
        Theme::Terrain => base,
        Theme::Plates => {
            let Some(grid) = &world.plate_ids else {
                return base;
            };
            let idx = grid.at(x, y) as usize;
            let color = world
                .plates
                .iter()
                .find(|p| p.plate_id as usize == idx)
                .and_then(|p| {
                    let bytes = p
                        .color_rgb
                        .split(',')
                        .map(|x| x.trim().parse::<u8>().ok())
                        .collect::<Vec<_>>();
                    if bytes.len() == 3 {
                        Some([bytes[0]?, bytes[1]?, bytes[2]?])
                    } else {
                        None
                    }
                })
                .unwrap_or(PALETTE[idx % PALETTE.len()]);
            mix(base, color, 0.82)
        }
        Theme::Cratons => {
            let Some(grid) = &world.craton_ids else {
                return base;
            };
            let idx = grid.at(x, y) as usize;
            if idx == 0 {
                mix(base, [28, 42, 56], 0.23)
            } else {
                mix(base, PALETTE[(idx + 3) % PALETTE.len()], 0.90)
            }
        }
        Theme::Provinces => {
            let Some(grid) = &world.province_ids else {
                return base;
            };
            let idx = grid.at(x, y) as usize;
            mix(base, PALETTE[(idx * 3 + 2) % PALETTE.len()], 0.83)
        }
        Theme::Crust => world
            .crust_km
            .as_ref()
            .map(|g| mix(base, ramp(g.at(x, y), 5.0, 65.0), 0.85))
            .unwrap_or(base),
        Theme::Lithosphere => world
            .lithosphere_km
            .as_ref()
            .map(|g| mix(base, ramp(g.at(x, y), 10.0, 260.0), 0.85))
            .unwrap_or(base),
        Theme::HeatFlow => world
            .heatflow
            .as_ref()
            .map(|g| mix(base, ramp(g.at(x, y), 20.0, 150.0), 0.85))
            .unwrap_or(base),
    }
}

/// The exact same world grids feed flat and orthographic views.
pub fn atlas_image(world: &World, theme: Theme) -> egui::ColorImage {
    let width = 1024usize;
    let height = (width * world.height / world.width).max(1);
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let sx = (x * world.width / width).min(world.width - 1);
            let sy = (y * world.height / height).min(world.height - 1);
            let c = sample_rgb(world, theme, sx, sy);
            rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba)
}

pub fn inverse_globe(
    sx: f64,
    sy: f64,
    center_lon_deg: f64,
    center_lat_deg: f64,
) -> Option<(f64, f64, f64)> {
    let radius2 = sx * sx + sy * sy;
    if radius2 > 1.0 {
        return None;
    }
    let z = (1.0 - radius2).sqrt();
    let lat0 = center_lat_deg.to_radians();
    let lon0 = center_lon_deg.to_radians();
    let lat = (sy * lat0.cos() + z * lat0.sin()).clamp(-1.0, 1.0).asin();
    let lon = lon0 + sx.atan2(z * lat0.cos() - sy * lat0.sin());
    let lon_deg = (lon.to_degrees() + 180.0).rem_euclid(360.0) - 180.0;
    Some((lon_deg, lat.to_degrees(), z))
}

pub fn forward_globe(lon: f64, lat: f64, center_lon: f64, center_lat: f64) -> Option<(f32, f32)> {
    let delta = (lon - center_lon).to_radians();
    let phi = lat.to_radians();
    let phi0 = center_lat.to_radians();
    let x = phi.cos() * delta.sin();
    let y = phi.sin() * phi0.cos() - phi.cos() * phi0.sin() * delta.cos();
    let z = phi.sin() * phi0.sin() + phi.cos() * phi0.cos() * delta.cos();
    if z <= 0.0 {
        None
    } else {
        Some((x as f32, y as f32))
    }
}

pub fn globe_image(
    world: &World,
    theme: Theme,
    center_lon: f64,
    center_lat: f64,
) -> egui::ColorImage {
    const SIZE: usize = 512;
    let mut rgba = Vec::with_capacity(SIZE * SIZE * 4);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let sx = ((x as f64 + 0.5) / SIZE as f64) * 2.0 - 1.0;
            let sy = 1.0 - ((y as f64 + 0.5) / SIZE as f64) * 2.0;
            if let Some((lon, lat, z)) = inverse_globe(sx, sy, center_lon, center_lat) {
                let (u, v) = geo_to_uv(lon, lat);
                let ix = ((u * world.width as f64).floor() as usize) % world.width;
                let iy = ((v * world.height as f64).floor() as usize).min(world.height - 1);
                let mut c = sample_rgb(world, theme, ix, iy);
                let light = (0.22 * sx + 0.22 * sy + 0.92 * z).max(0.25) as f32;
                for ch in &mut c {
                    *ch = ((*ch as f32) * light).clamp(0.0, 255.0) as u8;
                }
                rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
            } else {
                rgba.extend_from_slice(&[17, 23, 33, 255]);
            }
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([SIZE, SIZE], &rgba)
}

pub fn atlas_geo_from_fraction(u: f64, v: f64) -> (f64, f64) {
    uv_to_geo(u, v)
}

pub fn globe_geo_from_fraction(
    u: f64,
    v: f64,
    center_lon: f64,
    center_lat: f64,
) -> Option<(f64, f64)> {
    let (lon, lat, _) = inverse_globe(2.0 * u - 1.0, 1.0 - 2.0 * v, center_lon, center_lat)?;
    Some((lon, lat))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globe_center_is_selected_longitude_and_latitude() {
        let (lon, lat, _) = inverse_globe(0.0, 0.0, 55.0, -28.0).unwrap();
        assert!((lon - 55.0).abs() < 1e-9);
        assert!((lat + 28.0).abs() < 1e-9);
        assert!(inverse_globe(1.0, 1.0, 0.0, 0.0).is_none());
    }

    #[test]
    fn globe_forward_inverse_roundtrip() {
        let (x, y) = forward_globe(24.0, 35.0, 0.0, 0.0).unwrap();
        let (lon, lat, _) = inverse_globe(x as f64, y as f64, 0.0, 0.0).unwrap();
        assert!((lon - 24.0).abs() < 0.0001);
        assert!((lat - 35.0).abs() < 0.0001);
    }

    #[test]
    fn color_ramp_is_bounded() {
        assert_eq!(elevation_color(-10000.0), [8, 32, 67]);
        assert_eq!(ramp(0.0, 0.0, 1.0), [32, 93, 150]);
    }

    #[test]
    fn no_reference_world_is_renderable() {
        let w = World::empty(std::path::PathBuf::new());
        let img = atlas_image(&w, Theme::Terrain);
        assert_eq!(img.size[0], 1024);
        assert!(img.size[1] > 0);
    }
}
