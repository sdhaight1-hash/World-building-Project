//! Shared map and orthographic globe projections, with data-driven thematic styling.
//! Scientific source fields and category boundaries are styled by cartography.rs.
//! The globe is a projection of the same fields, not a separately painted texture.
use crate::{
    cartography::Cartography,
    world::{geo_to_uv, uv_to_geo, World},
};
use eframe::egui;
use std::{f64::consts::PI, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Terrain,
    Slope,
    Plates,
    Boundaries,
    PlateConvergence,
    PlateDivergence,
    PlateShear,
    Cratons,
    BasementAge,
    Provinces,
    Crust,
    Lithosphere,
    HeatFlow,
    RainShadow,
}

impl Theme {
    pub const ALL: [Self; 14] = [
        Self::Terrain,
        Self::Slope,
        Self::Plates,
        Self::Boundaries,
        Self::PlateConvergence,
        Self::PlateDivergence,
        Self::PlateShear,
        Self::Cratons,
        Self::BasementAge,
        Self::Provinces,
        Self::Crust,
        Self::Lithosphere,
        Self::HeatFlow,
        Self::RainShadow,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Terrain => "Physical relief + bathymetry",
            Self::Slope => "Surface slope",
            Self::Plates => "Tectonic plates",
            Self::Boundaries => "Plate boundary classes",
            Self::PlateConvergence => "Plate convergence",
            Self::PlateDivergence => "Plate spreading",
            Self::PlateShear => "Plate shear",
            Self::Cratons => "Cratonic basement",
            Self::BasementAge => "Basement age",
            Self::Provinces => "Crustal provinces",
            Self::Crust => "Crust thickness",
            Self::Lithosphere => "Lithosphere depth",
            Self::HeatFlow => "Surface heat flow",
            Self::RainShadow => "Rain shadow",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Terrain => "physical-relief",
            Self::Slope => "surface-slope",
            Self::Plates => "tectonic-plates",
            Self::Boundaries => "plate-boundaries",
            Self::PlateConvergence => "plate-convergence",
            Self::PlateDivergence => "plate-spreading",
            Self::PlateShear => "plate-shear",
            Self::Cratons => "cratons",
            Self::BasementAge => "basement-age",
            Self::Provinces => "crustal-provinces",
            Self::Crust => "crust-thickness",
            Self::Lithosphere => "lithosphere-depth",
            Self::HeatFlow => "heat-flow",
            Self::RainShadow => "rain-shadow",
        }
    }

    pub fn available(self, w: &World) -> bool {
        match self {
            Self::Terrain => w.elevation.is_some(),
            Self::Slope => w.slope_deg.is_some(),
            Self::Plates | Self::Boundaries => w.plate_ids.is_some(),
            Self::PlateConvergence | Self::PlateDivergence | Self::PlateShear => {
                w.plate_motion.is_some()
            },
            Self::Cratons => w.craton_ids.is_some(),
            Self::BasementAge => w.craton_ids.is_some() && !w.cratons.is_empty(),
            Self::Provinces => w.province_ids.is_some(),
            Self::Crust => w.crust_km.is_some(),
            Self::Lithosphere => w.lithosphere_km.is_some(),
            Self::HeatFlow => w.heatflow.is_some(),
            Self::RainShadow => w.rain_shadow.is_some(),
        }
    }

    pub fn units(self) -> &'static str {
        match self {
            Self::Terrain => "m above/below datum",
            Self::Slope => "degrees",
            Self::BasementAge => "Ga",
            Self::Crust | Self::Lithosphere => "km",
            Self::HeatFlow => "mW/m²",
            Self::RainShadow => "signed dimensionless index",
            Self::PlateConvergence | Self::PlateDivergence | Self::PlateShear => "cm/year",
            _ => "named categories",
        }
    }
}

/// Full native-resolution RGBA texture. Not pre-rendered externally.
/// No nearest-neighbor 1024px downscaling of the scientific source grid.
pub fn atlas_image(world: &World, art: &Cartography, theme: Theme) -> egui::ColorImage {
    let (width, height) = art.size();
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let c = art.sample(world, theme, x, y);
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
    art: &Cartography,
    theme: Theme,
    center_lon: f64,
    center_lat: f64,
) -> egui::ColorImage {
    const SIZE: usize = 896;
    let mut rgba = Vec::with_capacity(SIZE * SIZE * 4);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let sx = ((x as f64 + 0.5) / SIZE as f64) * 2.0 - 1.0;
            let sy = 1.0 - ((y as f64 + 0.5) / SIZE as f64) * 2.0;
            if let Some((lon, lat, z)) = inverse_globe(sx, sy, center_lon, center_lat) {
                let (u, v) = geo_to_uv(lon, lat);
                let ix = ((u * world.width as f64).floor() as usize) % world.width;
                let iy = ((v * world.height as f64).floor() as usize).min(world.height - 1);
                let mut c = art.sample(world, theme, ix, iy);
                let light = (0.53 + 0.5 * (0.22 * sx + 0.24 * sy + 0.92 * z)) as f32;
                for ch in &mut c {
                    *ch = ((*ch as f32) * light).clamp(0.0, 255.0) as u8;
                }
                rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
            } else {
                rgba.extend_from_slice(&[16, 24, 35, 255]);
            }
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([SIZE, SIZE], &rgba)
}

/// Image and machine-readable legend are exported from the same style used
/// by the interactive atlas. No colour-only PNG is mislabeled as geodata.
pub fn export_atlas_png(
    world: &World,
    art: &Cartography,
    theme: Theme,
    path: &Path,
) -> Result<(), String> {
    let image = atlas_image(world, art, theme);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut bytes = Vec::with_capacity(image.pixels.len() * 4);
    for pixel in &image.pixels {
        bytes.extend_from_slice(&pixel.to_array());
    }
    image::save_buffer(
        path,
        &bytes,
        image.size[0] as u32,
        image.size[1] as u32,
        image::ColorType::Rgba8,
    )
    .map_err(|e| format!("{}: {e}", path.display()))?;
    let legend = art
        .legend(world, theme)
        .into_iter()
        .map(|it| {
            serde_json::json!({
                "name":it.label,"rgb":it.rgb,
            })
        })
        .collect::<Vec<_>>();
    let meta = serde_json::json!({
        "map":theme.name(),"units":theme.units(),
        "width":world.width,"height":world.height,
        "style":"Aegaea Studio data-driven cartography v1",
        "coordinate_convention":{
            "lon_at_x":"180 - 360*(x+0.5)/width",
            "lat_at_y":"-90 + 180*(y+0.5)/height",
            "not_Earth_EPSG_4326":true,
        },
        "derived_from":"native TIFF/CSV geology and classified GeoJSON boundaries",
        "legend":legend,
        "scientific_status":"synthetic fictional world; terrain shading and classification, not geological reconstruction",
    });
    std::fs::write(
        path.with_extension("json"),
        serde_json::to_vec_pretty(&meta).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
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
    use std::path::PathBuf;

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
    fn exported_map_has_native_dimensions_and_legend() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/aegaea/tectonics/v1");
        let world = World::load(root).expect("reference source data");
        let art = Cartography::new(&world);
        let image = atlas_image(&world, &art, Theme::Boundaries);
        assert_eq!(image.size, [1672, 941]);
        let path =
            std::env::temp_dir().join(format!("aegaea-cartography-{}.png", std::process::id()));
        export_atlas_png(&world, &art, Theme::Boundaries, &path).unwrap();
        let loaded = image::open(&path).unwrap();
        assert_eq!((loaded.width(), loaded.height()), (1672, 941));
        let meta = std::fs::read_to_string(path.with_extension("json")).unwrap();
        assert!(meta.contains("Subduction"));
        assert!(meta.contains("Divergent spreading"));
        let _ = std::fs::remove_file(path.with_extension("json"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn slope_map_uses_derived_numeric_field_at_native_resolution() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/aegaea/tectonics/v1");
        let mut world = World::load(root).unwrap();
        world.slope_deg = world
            .elevation
            .as_ref()
            .map(|dem| crate::map_graph::slope_degrees(dem, 6371.0).unwrap());
        let art = Cartography::new(&world);
        assert!(Theme::Slope.available(&world));
        assert_eq!(Theme::Slope.units(), "degrees");
        assert_eq!(art.legend(&world, Theme::Slope).len(), 6);
        let image = atlas_image(&world, &art, Theme::Slope);
        assert_eq!(image.size, [1672, 941]);
        assert!(world
            .slope_deg
            .as_ref()
            .unwrap()
            .cells
            .iter()
            .any(|v| *v > 0.0));
    }

    #[test]
    fn blank_world_does_not_need_aegaea_assets() {
        let world = World::empty(PathBuf::from("not-aegaea"));
        let art = Cartography::new(&world);
        let result = atlas_image(&world, &art, Theme::Terrain);
        assert_eq!(result.size, [1024, 512]);
    }

    #[test]
    fn planet_longitude_seam_is_periodic() {
        let lon0 = crate::world::geo_to_uv(180.0, 0.0).0;
        let lon1 = crate::world::geo_to_uv(-180.0, 0.0).0;
        assert!((lon0 - lon1).abs() < 1e-10);
        assert!((PI - std::f64::consts::PI).abs() < 1e-10);
    }
}
