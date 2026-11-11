//! Shared map and orthographic globe projections, with data-driven thematic styling.
//! Scientific source fields and category boundaries are styled by cartography.rs.
//! The globe is a projection of the same fields, not a separately painted texture.
use crate::{
    cartography::Cartography,
    datasets::{month_name, ClimateField, MonthlyField},
    world::{uv_to_geo, World},
};
use eframe::egui;
use std::path::Path;

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
    MonthlyTemperature,
    MonthlyPrecipitation,
    AnnualTemperature,
    AnnualPrecipitation,
    MonthlyInsolation,
    MonthlyPressure,
    MonthlySpecificHumidity,
    MonthlyWindEast,
    MonthlyWindNorth,
    SurfaceHeatCapacity,
    TransportedVaporColumn,
    TransportedPressure,
    TransportedSpecificHumidity,
    EvolvingWindEast,
    EvolvingWindNorth,
}

impl Theme {
    pub const ALL: [Self; 29] = [
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
        Self::MonthlyTemperature,
        Self::MonthlyPrecipitation,
        Self::AnnualTemperature,
        Self::AnnualPrecipitation,
        Self::MonthlyInsolation,
        Self::MonthlyPressure,
        Self::MonthlySpecificHumidity,
        Self::MonthlyWindEast,
        Self::MonthlyWindNorth,
        Self::SurfaceHeatCapacity,
        Self::TransportedVaporColumn,
        Self::TransportedPressure,
        Self::TransportedSpecificHumidity,
        Self::EvolvingWindEast,
        Self::EvolvingWindNorth,
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
            Self::MonthlyTemperature => "Monthly air temperature",
            Self::MonthlyPrecipitation => "Monthly precipitation",
            Self::AnnualTemperature => "Annual air temperature · day-weighted mean",
            Self::AnnualPrecipitation => "Annual precipitation total",
            Self::MonthlyInsolation => "Monthly top-of-atmosphere sunlight",
            Self::MonthlyPressure => "Monthly surface pressure",
            Self::MonthlySpecificHumidity => "Monthly specific humidity",
            Self::MonthlyWindEast => "Monthly wind · eastward component",
            Self::MonthlyWindNorth => "Monthly wind · northward component",
            Self::SurfaceHeatCapacity => "Surface areal heat capacity",
            Self::TransportedVaporColumn => "Transported vapor-column proxy",
            Self::TransportedPressure => "Pressure after transport · diagnostic",
            Self::TransportedSpecificHumidity => "Humidity after transport · diagnostic",
            Self::EvolvingWindEast => "Wind after dynamics · eastward",
            Self::EvolvingWindNorth => "Wind after dynamics · northward",
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
            Self::MonthlyTemperature => "monthly-air-temperature",
            Self::MonthlyPrecipitation => "monthly-precipitation",
            Self::AnnualTemperature => "annual-air-temperature",
            Self::AnnualPrecipitation => "annual-precipitation",
            Self::MonthlyInsolation => "monthly-toa-insolation",
            Self::MonthlyPressure => "monthly-surface-pressure",
            Self::MonthlySpecificHumidity => "monthly-specific-humidity",
            Self::MonthlyWindEast => "monthly-wind-east",
            Self::MonthlyWindNorth => "monthly-wind-north",
            Self::SurfaceHeatCapacity => "surface-heat-capacity",
            Self::TransportedVaporColumn => "transported-vapor-column",
            Self::TransportedPressure => "transported-pressure-diagnostic",
            Self::TransportedSpecificHumidity => "transported-humidity-diagnostic",
            Self::EvolvingWindEast => "evolving-wind-east",
            Self::EvolvingWindNorth => "evolving-wind-north",
        }
    }

    pub fn available(self, w: &World) -> bool {
        match self {
            Self::Terrain => w.elevation.is_some(),
            Self::Slope => w.slope_deg.is_some(),
            Self::Plates | Self::Boundaries => w.plate_ids.is_some(),
            Self::PlateConvergence | Self::PlateDivergence | Self::PlateShear => {
                w.plate_motion.is_some()
            }
            Self::Cratons => w.craton_ids.is_some(),
            Self::BasementAge => w.craton_ids.is_some() && !w.cratons.is_empty(),
            Self::Provinces => w.province_ids.is_some(),
            Self::Crust => w.crust_km.is_some(),
            Self::Lithosphere => w.lithosphere_km.is_some(),
            Self::HeatFlow => w.heatflow.is_some(),
            Self::SurfaceHeatCapacity => w.surface_heat_capacity.is_some(),
            Self::RainShadow => w.rain_shadow.is_some(),
            Self::MonthlyTemperature
            | Self::MonthlyPrecipitation
            | Self::AnnualTemperature
            | Self::AnnualPrecipitation
            | Self::MonthlyInsolation
            | Self::MonthlyPressure
            | Self::MonthlySpecificHumidity
            | Self::MonthlyWindEast
            | Self::MonthlyWindNorth
            | Self::TransportedVaporColumn
            | Self::TransportedPressure
            | Self::TransportedSpecificHumidity
            | Self::EvolvingWindEast
            | Self::EvolvingWindNorth => self
                .scientific_id()
                .is_some_and(|id| w.scientific_fields.contains_key(id)),
        }
    }

    pub fn monthly_source(self) -> Option<MonthlyField> {
        match self {
            Self::MonthlyTemperature => Some(MonthlyField::Temperature),
            Self::MonthlyPrecipitation => Some(MonthlyField::Precipitation),
            Self::MonthlyPressure => Some(MonthlyField::Pressure),
            Self::MonthlySpecificHumidity => Some(MonthlyField::SpecificHumidity),
            Self::MonthlyWindEast => Some(MonthlyField::WindEast),
            Self::MonthlyWindNorth => Some(MonthlyField::WindNorth),
            _ => None,
        }
    }

    pub fn annual_field(self) -> Option<ClimateField> {
        match self {
            Self::AnnualTemperature => Some(ClimateField::Temperature),
            Self::AnnualPrecipitation => Some(ClimateField::Precipitation),
            _ => None,
        }
    }

    pub fn scientific_id(self) -> Option<&'static str> {
        if self == Self::MonthlyInsolation {
            Some(crate::planetary::INSOLATION_ID)
        } else if self == Self::TransportedVaporColumn {
            Some(crate::moisture_transport::WATER_COLUMN_ID)
        } else if self == Self::TransportedPressure {
            Some(crate::moisture_transport::PRESSURE_PREVIEW_ID)
        } else if self == Self::TransportedSpecificHumidity {
            Some(crate::moisture_transport::HUMIDITY_PREVIEW_ID)
        } else if self == Self::EvolvingWindEast {
            Some(crate::atmosphere_dynamics::WIND_EAST_PREVIEW_ID)
        } else if self == Self::EvolvingWindNorth {
            Some(crate::atmosphere_dynamics::WIND_NORTH_PREVIEW_ID)
        } else {
            self.monthly_source()
                .map(|kind| kind.id())
                .or_else(|| self.annual_field().map(|kind| kind.annual_id()))
        }
    }

    pub fn is_transport_preview(self) -> bool {
        matches!(
            self,
            Self::TransportedVaporColumn
                | Self::TransportedPressure
                | Self::TransportedSpecificHumidity
                | Self::EvolvingWindEast
                | Self::EvolvingWindNorth
        )
    }

    pub fn is_monthly(self) -> bool {
        self.monthly_source().is_some()
            || self == Self::MonthlyInsolation
            || self.is_transport_preview()
    }

    pub fn units(self) -> &'static str {
        match self {
            Self::Terrain => "m above/below datum",
            Self::Slope => "degrees",
            Self::BasementAge => "Ga",
            Self::Crust | Self::Lithosphere => "km",
            Self::HeatFlow => "mW/m²",
            Self::RainShadow => "signed dimensionless index",
            Self::MonthlyTemperature => "°C",
            Self::MonthlyPrecipitation => "mm/month",
            Self::AnnualTemperature => "°C",
            Self::AnnualPrecipitation => "mm/year",
            Self::MonthlyInsolation => "W/m²",
            Self::MonthlyPressure => "hPa",
            Self::MonthlySpecificHumidity => "g/kg",
            Self::MonthlyWindEast | Self::MonthlyWindNorth => "m/s",
            Self::SurfaceHeatCapacity => "MJ/(m²·K)",
            Self::TransportedVaporColumn => "kg/m²",
            Self::TransportedPressure => "hPa",
            Self::TransportedSpecificHumidity => "g/kg",
            Self::EvolvingWindEast | Self::EvolvingWindNorth => "m/s",
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
    let monthly_source = theme.monthly_source();
    let scientific_id = theme.scientific_id();
    let provenance = scientific_id.and_then(|id| world.scientific_provenance.get(id));
    let meta = serde_json::json!({
        "map":theme.name(),"units":theme.units(),
        "width":world.width,"height":world.height,
        "style":"Aegaea Studio data-driven cartography v1",
        "coordinate_convention":{
            "lon_at_x":"180 - 360*(x+0.5)/width",
            "lat_at_y":"-90 + 180*(y+0.5)/height",
            "not_Earth_EPSG_4326":true,
        },
        "dataset_id":if theme == Theme::SurfaceHeatCapacity {
            Some("surface.areal_heat_capacity")
        } else {
            scientific_id
        },
        "month":theme.is_monthly().then_some(world.active_month),
        "month_name":theme.is_monthly().then(|| month_name(world.active_month)),
        "planetary_calendar":(theme == Theme::MonthlyInsolation || theme == Theme::AnnualTemperature).then_some(&world.calendar),
        "moisture_transport_budget":theme.is_transport_preview().then_some(&world.moisture_transport_budget),
        "provenance":if theme == Theme::SurfaceHeatCapacity {
            serde_json::json!({"kind":"authored","note":"Project-local hand-authored stationary heat capacity; not derived from land/ocean mask"})
        } else {
            serde_json::json!(provenance)
        },
        "derived_from":if theme.is_transport_preview() && world.moisture_transport_budget.as_ref().and_then(|b| b.dynamics.as_ref()).is_some_and(|d| d.momentum.is_some()) {
            "spherical barotropic mass and vapor continuity; conservative global absolute axial angular momentum, paired pressure torques, curvature-aware meridional momentum, energy drift audit"
        } else if theme.is_transport_preview() && world.moisture_transport_budget.as_ref().is_some_and(|b| b.dynamics.is_some()) {
            "linearized barotropic pressure-gradient tendency and exact local Coriolis rotation, inferred prograde spin, then conservative coupled spherical air-vapor finite-volume transport"
        } else if theme.is_transport_preview() {
            "same coupled finite-volume fluxes advect hydrostatic moist-air mass and water vapor; prescribed wind, planetary radius and surface gravity"
        } else if theme == Theme::SurfaceHeatCapacity {
            "project-local explicitly authored surface areal heat capacity; land/ocean status is not inferred"
        } else if theme == Theme::MonthlyInsolation {
            "planetary calendar and circular-orbit solar geometry; no atmosphere or climate solver"
        } else if theme.annual_field().is_some() {
            "calculated from all twelve authored monthly source slices; see provenance"
        } else if monthly_source.is_some() {
            "project-local monthly scientific dataset; see provenance"
        } else {
            "native map fields, source geology and/or authored geometry"
        },
        "legend":legend,
        "scientific_status":if theme.is_transport_preview() && world.moisture_transport_budget.as_ref().and_then(|b| b.dynamics.as_ref()).is_some_and(|d| d.momentum.is_some()) {
            "experimental spherical barotropic-column momentum finite volume; global air, vapor and axial angular momentum budgets enforced; first-order energy dissipative, polar faces closed, no full primitive-equation circulation or rainfall prediction"
        } else if theme.is_transport_preview() && world.moisture_transport_budget.as_ref().is_some_and(|b| b.dynamics.is_some()) {
            "experimental linearized pressure-gradient and Coriolis wind tendencies coupled to air-vapor mass conservation; no nonlinear momentum advection, spherical metric forces, vertical structure, energy closure, rainfall or full circulation model"
        } else if theme.is_transport_preview() {
            "read-only coupled air/water continuity with diagnostic pressure and humidity; prescribed winds only, no momentum, energy, vertical atmosphere, rainfall, or climate simulation"
        } else if theme == Theme::SurfaceHeatCapacity {
            "author-specified areal heat capacity input, not computed ocean or soil thermal physics"
        } else if theme == Theme::MonthlyInsolation {
            "daily-mean top-of-atmosphere irradiance integrated over configured month; circular orbit, no weather or climate model"
        } else if theme.annual_field().is_some() {
            "deterministic monthly aggregation, not a climate model; annual temperature weighted by calendar solar days"
        } else if monthly_source.is_some() {
            "source-defined values; no precipitation or temperature simulation is implied"
        } else {
            "fictional world scientific and authored fields; provenance must be checked per source"
        },
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
    fn euler_motion_maps_render_from_numeric_source_without_old_dem() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/aegaea/tectonics/v1");
        let mut world = World::load(root).unwrap();
        world.elevation = None;
        world.plate_motion = crate::plate_kinematics::derive(&world, 6371.0).unwrap();
        let art = Cartography::new(&world);
        for theme in [
            Theme::PlateConvergence,
            Theme::PlateDivergence,
            Theme::PlateShear,
        ] {
            assert!(theme.available(&world));
            assert_eq!(theme.units(), "cm/year");
            assert_eq!(art.legend(&world, theme).len(), 5);
            let map = atlas_image(&world, &art, theme);
            assert_eq!(map.size, [1672, 941]);
            let field = match theme {
                Theme::PlateConvergence => &world.plate_motion.as_ref().unwrap().convergence_cm_yr,
                Theme::PlateDivergence => &world.plate_motion.as_ref().unwrap().divergence_cm_yr,
                Theme::PlateShear => &world.plate_motion.as_ref().unwrap().shear_cm_yr,
                _ => unreachable!(),
            };
            assert!(field.cells.iter().any(|&value| value > 1.0));
        }
    }

    #[test]
    fn independently_authored_world_renders_all_maps_except_opt_in_transport() {
        use crate::map_layers::{LayerKind, ProjectLayers};
        use crate::world::Grid;

        let mut world = World::empty(PathBuf::from("independent"));
        world.width = 64;
        world.height = 32;
        world.elevation = Some(Grid::new(64, 32, -2_000.0));
        let mut layers = ProjectLayers::default();
        for kind in LayerKind::ALL {
            layers
                .create(&mut world, kind, kind.default_fill())
                .unwrap();
        }
        layers.add_plate(&mut world).unwrap();
        layers.add_craton(&mut world).unwrap();
        layers
            .paint(&mut world, LayerKind::Plates, 0.0, 0.0, 9, 2.0)
            .unwrap();
        layers
            .paint(&mut world, LayerKind::Cratons, 0.0, 0.0, 5, 1.0)
            .unwrap();
        layers
            .paint(&mut world, LayerKind::Provinces, 20.0, 0.0, 8, 3.0)
            .unwrap();
        for (i, plate) in layers.plates.as_mut().unwrap().iter_mut().enumerate() {
            plate.euler_pole_lon_deg = Some(i as f64 * 45.0);
            plate.euler_pole_lat_deg = Some(15.0);
            plate.signed_rate_deg_per_ma = Some(if i == 0 { 0.0 } else { 2.0 });
        }
        world.plates.clone_from(layers.plates.as_ref().unwrap());
        world.plate_motion = crate::plate_kinematics::derive(&world, 6371.0).unwrap();
        world.slope_deg = Some(
            crate::map_graph::slope_degrees(world.elevation.as_ref().unwrap(), 6371.0).unwrap(),
        );
        world.rain_shadow = Some(crate::rainshadow::derive(
            world.elevation.as_ref().unwrap(),
            crate::rainshadow::WindFrom::West,
        ));
        let mut monthly = crate::datasets::DatasetRegistry::default();
        for month in 1..=12 {
            for kind in crate::datasets::MonthlyField::ALL {
                monthly
                    .create_month(&mut world, kind, month, kind.default_fill())
                    .unwrap();
            }
        }
        let art = Cartography::new(&world);
        assert!(
            !Theme::TransportedVaporColumn.available(&world),
            "An opt-in numerical preview cannot exist before it is computed"
        );
        for theme in Theme::ALL {
            if theme.is_transport_preview() {
                continue;
            }
            assert!(theme.available(&world), "{} should be usable", theme.name());
            assert_eq!(atlas_image(&world, &art, theme).size, [64, 32]);
            assert!(
                !art.legend(&world, theme).is_empty(),
                "{} should include a legend",
                theme.name()
            );
        }
    }

    #[test]
    fn monthly_climate_atlas_and_provenance_export_are_source_aware() {
        let mut world = World::empty(PathBuf::from("independent-climate"));
        world.width = 48;
        world.height = 24;
        let mut datasets = crate::datasets::DatasetRegistry::default();
        datasets
            .create_month(&mut world, ClimateField::Temperature, 2, -5.0)
            .unwrap();
        datasets
            .create_month(&mut world, ClimateField::Precipitation, 2, 180.0)
            .unwrap();
        let art = Cartography::new(&world);
        for theme in [Theme::MonthlyTemperature, Theme::MonthlyPrecipitation] {
            assert!(theme.available(&world));
            assert_eq!(atlas_image(&world, &art, theme).size, [48, 24]);
            assert!(art.legend(&world, theme).len() >= 6);
            let path = std::env::temp_dir().join(format!(
                "aegaea-monthly-{}-{}.png",
                std::process::id(),
                theme.slug()
            ));
            export_atlas_png(&world, &art, theme, &path).unwrap();
            let metadata = std::fs::read_to_string(path.with_extension("json")).unwrap();
            assert!(metadata.contains(r#""month": 2"#));
            assert!(metadata.contains("author"));
            assert!(metadata.contains(theme.monthly_source().unwrap().id()));
            std::fs::remove_file(path.with_extension("json")).unwrap();
            std::fs::remove_file(path).unwrap();
        }
        datasets.activate(&mut world, 3).unwrap();
        assert!(!Theme::MonthlyTemperature.available(&world));
        assert!(!Theme::MonthlyPrecipitation.available(&world));
    }

    #[test]
    fn atmospheric_maps_export_correct_month_units_and_authored_source() {
        use crate::datasets::MonthlyField;
        let mut world = World::empty(PathBuf::from("atmosphere"));
        world.width = 48;
        world.height = 24;
        let mut registry = crate::datasets::DatasetRegistry::default();
        for field in [
            MonthlyField::Pressure,
            MonthlyField::SpecificHumidity,
            MonthlyField::WindEast,
            MonthlyField::WindNorth,
        ] {
            registry
                .create_month(&mut world, field, 3, field.default_fill())
                .unwrap();
        }
        registry
            .paint(&mut world, MonthlyField::WindEast, 3, 0.0, 0.0, 2, -22.0)
            .unwrap();
        let art = Cartography::new(&world);
        for theme in [
            Theme::MonthlyPressure,
            Theme::MonthlySpecificHumidity,
            Theme::MonthlyWindEast,
            Theme::MonthlyWindNorth,
        ] {
            assert!(theme.available(&world));
            assert_eq!(atlas_image(&world, &art, theme).size, [48, 24]);
            assert_eq!(art.legend(&world, theme).len(), 7);
            let path = std::env::temp_dir().join(format!(
                "aegaea-atmosphere-{}-{}.png",
                std::process::id(),
                theme.slug()
            ));
            export_atlas_png(&world, &art, theme, &path).unwrap();
            let meta: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path.with_extension("json")).unwrap())
                    .unwrap();
            assert_eq!(meta["month"], 3);
            assert_eq!(meta["dataset_id"], theme.scientific_id().unwrap());
            assert_eq!(meta["units"], theme.units());
            assert_eq!(meta["provenance"]["kind"], "authored");
            std::fs::remove_file(path.with_extension("json")).unwrap();
            std::fs::remove_file(path).unwrap();
        }
        registry.activate(&mut world, 4).unwrap();
        for theme in [
            Theme::MonthlyPressure,
            Theme::MonthlySpecificHumidity,
            Theme::MonthlyWindEast,
            Theme::MonthlyWindNorth,
        ] {
            assert!(
                !theme.available(&world),
                "Missing months must not be inferred"
            );
        }
    }

    #[test]
    fn coupled_transport_preview_exports_all_three_maps_and_dual_mass_budget() {
        use crate::{
            datasets::{DatasetRegistry, MonthlyField},
            moisture_transport::{self, HUMIDITY_PREVIEW_ID, PRESSURE_PREVIEW_ID, WATER_COLUMN_ID},
        };
        let mut world = World::empty(PathBuf::from("transport-export"));
        world.width = 48;
        world.height = 24;
        let mut sources = DatasetRegistry::default();
        for (kind, value) in [
            (MonthlyField::Pressure, 1013.25),
            (MonthlyField::SpecificHumidity, 12.0),
            (MonthlyField::WindEast, 20.0),
            (MonthlyField::WindNorth, 0.0),
        ] {
            sources.create_month(&mut world, kind, 4, value).unwrap();
        }
        // The source grids are immutable under numerical transport.
        let source_p = world.scientific_fields[MonthlyField::Pressure.id()]
            .cells
            .clone();
        let source_q = world.scientific_fields[MonthlyField::SpecificHumidity.id()]
            .cells
            .clone();
        for theme in [
            Theme::TransportedVaporColumn,
            Theme::TransportedPressure,
            Theme::TransportedSpecificHumidity,
        ] {
            assert!(!theme.available(&world));
        }
        let result = moisture_transport::preview(&world, 6371.0, 9.80665, 120.0).unwrap();
        moisture_transport::publish(&mut world, result);
        let art = Cartography::new(&world);
        for (theme, id, unit) in [
            (Theme::TransportedVaporColumn, WATER_COLUMN_ID, "kg/m²"),
            (Theme::TransportedPressure, PRESSURE_PREVIEW_ID, "hPa"),
            (
                Theme::TransportedSpecificHumidity,
                HUMIDITY_PREVIEW_ID,
                "g/kg",
            ),
        ] {
            assert!(theme.available(&world));
            assert_eq!(theme.scientific_id(), Some(id));
            assert_eq!(theme.units(), unit);
            assert_eq!(art.legend(&world, theme).len(), 7);
            assert_eq!(atlas_image(&world, &art, theme).size, [48, 24]);
            let path = std::env::temp_dir().join(format!(
                "aegaea-coupled-{}-{}.png",
                std::process::id(),
                theme.slug()
            ));
            export_atlas_png(&world, &art, theme, &path).unwrap();
            let meta: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path.with_extension("json")).unwrap())
                    .unwrap();
            assert_eq!(meta["month"], 4);
            assert_eq!(meta["dataset_id"], id);
            assert_eq!(meta["units"], unit);
            assert_eq!(meta["provenance"]["kind"], "derived");
            assert_eq!(meta["moisture_transport_budget"]["month"], 4);
            assert_eq!(
                meta["moisture_transport_budget"]["surface_gravity_m_s2"],
                9.80665
            );
            assert!(meta["moisture_transport_budget"]["air_kg_before"].is_number());
            assert!(
                meta["moisture_transport_budget"]["air_relative_mass_error"]
                    .as_f64()
                    .unwrap()
                    < 1.0e-7
            );
            assert!(
                meta["moisture_transport_budget"]["relative_mass_error"]
                    .as_f64()
                    .unwrap()
                    < 1.0e-7
            );
            std::fs::remove_file(path.with_extension("json")).unwrap();
            std::fs::remove_file(path).unwrap();
        }
        assert_eq!(
            world.scientific_fields[MonthlyField::Pressure.id()].cells,
            source_p
        );
        assert_eq!(
            world.scientific_fields[MonthlyField::SpecificHumidity.id()].cells,
            source_q
        );
        sources.activate(&mut world, 5).unwrap();
        for theme in [
            Theme::TransportedVaporColumn,
            Theme::TransportedPressure,
            Theme::TransportedSpecificHumidity,
        ] {
            assert!(
                !theme.available(&world),
                "Old time slice must not be reused"
            );
        }
        assert!(world.moisture_transport_budget.is_none());
    }

    #[test]
    fn evolving_wind_maps_are_opt_in_export_coriolis_metadata_and_clear_on_month_switch() {
        use crate::{
            atmosphere_dynamics,
            datasets::{DatasetRegistry, MonthlyField},
            moisture_transport,
        };
        let mut w = World::empty(PathBuf::from("dynamic-export"));
        w.width = 36;
        w.height = 18;
        let mut authored = DatasetRegistry::default();
        for (kind, value) in [
            (MonthlyField::Pressure, 1013.25),
            (MonthlyField::SpecificHumidity, 8.0),
            (MonthlyField::WindEast, 24.0),
            (MonthlyField::WindNorth, 0.0),
        ] {
            authored.create_month(&mut w, kind, 4, value).unwrap();
        }
        assert!(!Theme::EvolvingWindEast.available(&w));
        assert!(!Theme::EvolvingWindNorth.available(&w));
        let original = w.scientific_fields[MonthlyField::WindEast.id()]
            .cells
            .clone();
        let derived = atmosphere_dynamics::preview(&w, 6371.0, 9.80665, 100.0, 60.0).unwrap();
        atmosphere_dynamics::publish(&mut w, derived);
        let art = Cartography::new(&w);
        for theme in [Theme::EvolvingWindEast, Theme::EvolvingWindNorth] {
            assert!(theme.available(&w));
            assert_eq!(theme.units(), "m/s");
            assert_eq!(art.legend(&w, theme).len(), 7);
            assert_eq!(atlas_image(&w, &art, theme).size, [36, 18]);
            let path = std::env::temp_dir().join(format!(
                "aegaea-dynamic-{}-{}.png",
                std::process::id(),
                theme.slug()
            ));
            export_atlas_png(&w, &art, theme, &path).unwrap();
            let meta: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path.with_extension("json")).unwrap())
                    .unwrap();
            assert_eq!(meta["month"], 4);
            assert_eq!(meta["dataset_id"], theme.scientific_id().unwrap());
            assert_eq!(meta["units"], "m/s");
            assert_eq!(meta["provenance"]["kind"], "derived");
            assert_eq!(
                meta["provenance"]["operator"],
                atmosphere_dynamics::OPERATOR
            );
            assert!(
                meta["moisture_transport_budget"]["dynamics"]["angular_velocity_rad_s"]
                    .as_f64()
                    .unwrap()
                    > 0.0
            );
            assert!(
                meta["moisture_transport_budget"]["relative_mass_error"]
                    .as_f64()
                    .unwrap()
                    < 1.0e-6
            );
            assert!(meta["scientific_status"]
                .as_str()
                .unwrap()
                .contains("momentum"));
            std::fs::remove_file(path.with_extension("json")).unwrap();
            std::fs::remove_file(path).unwrap();
        }
        assert_eq!(
            original,
            w.scientific_fields[MonthlyField::WindEast.id()].cells
        );
        authored.activate(&mut w, 5).unwrap();
        assert!(!Theme::EvolvingWindEast.available(&w));
        assert!(!Theme::EvolvingWindNorth.available(&w));
        assert!(!Theme::TransportedPressure.available(&w));
        assert!(w.moisture_transport_budget.is_none());
        assert!(moisture_transport::missing_sources(&w).len() >= 4);
    }

    #[test]
    fn spherical_momentum_maps_export_axial_and_energy_audits_without_source_overwrite() {
        use crate::{
            atmosphere_dynamics,
            datasets::{DatasetRegistry, MonthlyField},
            spherical_momentum,
        };
        let mut world = World::empty(PathBuf::from("spherical-map"));
        world.width = 36;
        world.height = 18;
        let mut source = DatasetRegistry::default();
        for (kind, value) in [
            (MonthlyField::Pressure, 1013.25),
            (MonthlyField::SpecificHumidity, 10.0),
            (MonthlyField::WindEast, 12.0),
            (MonthlyField::WindNorth, 8.0),
        ] {
            source.create_month(&mut world, kind, 3, value).unwrap();
        }
        let p_original = world.scientific_fields[MonthlyField::Pressure.id()]
            .cells
            .clone();
        let u_original = world.scientific_fields[MonthlyField::WindEast.id()]
            .cells
            .clone();
        assert!(!Theme::EvolvingWindEast.available(&world));
        let preview = spherical_momentum::preview(&world, 6371.0, 9.80665, 45.0, 60.0).unwrap();
        atmosphere_dynamics::publish(&mut world, preview);
        let art = Cartography::new(&world);
        for theme in [
            Theme::TransportedVaporColumn,
            Theme::TransportedPressure,
            Theme::TransportedSpecificHumidity,
            Theme::EvolvingWindEast,
            Theme::EvolvingWindNorth,
        ] {
            assert!(theme.available(&world));
            assert_eq!(atlas_image(&world, &art, theme).size, [36, 18]);
            let path = std::env::temp_dir().join(format!(
                "aegaea-spherical-{}-{}.png",
                std::process::id(),
                theme.slug()
            ));
            export_atlas_png(&world, &art, theme, &path).unwrap();
            let meta: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path.with_extension("json")).unwrap())
                    .unwrap();
            assert_eq!(meta["month"], 3);
            assert_eq!(meta["dataset_id"], theme.scientific_id().unwrap());
            assert_eq!(meta["provenance"]["kind"], "derived");
            assert_eq!(meta["provenance"]["operator"], spherical_momentum::OPERATOR);
            assert!(
                meta["moisture_transport_budget"]["dynamics"]["momentum"]
                    ["relative_axial_momentum_error"]
                    .as_f64()
                    .unwrap()
                    < 1.0e-6
            );
            assert!(meta["moisture_transport_budget"]["dynamics"]["momentum"]
                ["relative_energy_drift"]
                .is_number());
            assert!(meta["scientific_status"]
                .as_str()
                .unwrap()
                .contains("energy"));
            std::fs::remove_file(path.with_extension("json")).unwrap();
            std::fs::remove_file(path).unwrap();
        }
        assert_eq!(
            world.scientific_fields[MonthlyField::Pressure.id()].cells,
            p_original
        );
        assert_eq!(
            world.scientific_fields[MonthlyField::WindEast.id()].cells,
            u_original
        );
        source.activate(&mut world, 4).unwrap();
        assert!(!Theme::EvolvingWindEast.available(&world));
        assert!(!Theme::TransportedPressure.available(&world));
        assert!(world.moisture_transport_budget.is_none());
    }

    #[test]
    fn surface_heat_capacity_exports_explicit_static_provenance() {
        use crate::map_layers::{LayerKind, ProjectLayers};
        let mut world = World::empty(PathBuf::from("thermal"));
        world.width = 32;
        world.height = 16;
        assert!(!Theme::SurfaceHeatCapacity.available(&world));
        let mut layers = ProjectLayers::default();
        layers
            .create(&mut world, LayerKind::SurfaceHeatCapacity, 2.0)
            .unwrap();
        layers
            .paint(
                &mut world,
                LayerKind::SurfaceHeatCapacity,
                0.0,
                0.0,
                2,
                200.0,
            )
            .unwrap();
        assert!(Theme::SurfaceHeatCapacity.available(&world));
        let art = Cartography::new(&world);
        assert_eq!(art.legend(&world, Theme::SurfaceHeatCapacity).len(), 7);
        let path = std::env::temp_dir().join(format!("aegaea-thermal-{}.png", std::process::id()));
        export_atlas_png(&world, &art, Theme::SurfaceHeatCapacity, &path).unwrap();
        let meta: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path.with_extension("json")).unwrap()).unwrap();
        assert_eq!(meta["month"], serde_json::Value::Null);
        assert_eq!(meta["dataset_id"], "surface.areal_heat_capacity");
        assert_eq!(meta["units"], "MJ/(m²·K)");
        assert_eq!(meta["provenance"]["kind"], "authored");
        std::fs::remove_file(path.with_extension("json")).unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn annual_climate_exports_true_derived_provenance_and_correct_units() {
        let mut world = World::empty(PathBuf::from("annual"));
        world.width = 48;
        world.height = 24;
        let mut registry = crate::datasets::DatasetRegistry::default();
        for month in 1..=12u8 {
            registry
                .create_month(&mut world, ClimateField::Temperature, month, 20.0)
                .unwrap();
            registry
                .create_month(&mut world, ClimateField::Precipitation, month, 125.0)
                .unwrap();
        }
        let art = Cartography::new(&world);
        for theme in [Theme::AnnualTemperature, Theme::AnnualPrecipitation] {
            assert!(theme.available(&world));
            assert_eq!(atlas_image(&world, &art, theme).size, [48, 24]);
            assert_eq!(art.legend(&world, theme).len(), 7);
            let target = std::env::temp_dir().join(format!(
                "aegaea-annual-{}-{}.png",
                std::process::id(),
                theme.slug()
            ));
            export_atlas_png(&world, &art, theme, &target).unwrap();
            let meta = std::fs::read_to_string(target.with_extension("json")).unwrap();
            assert!(meta.contains("derived"));
            assert!(meta.contains("twelve"));
            assert!(meta.contains(theme.scientific_id().unwrap()));
            assert!(meta.contains(theme.units()));
            assert!(meta.contains(r#""month": null"#));
            std::fs::remove_file(target.with_extension("json")).unwrap();
            std::fs::remove_file(target).unwrap();
        }
        assert_eq!(
            world.scientific_fields[ClimateField::Temperature.annual_id()].at(0, 0),
            20.0
        );
        assert_eq!(
            world.scientific_fields[ClimateField::Precipitation.annual_id()].at(0, 0),
            1500.0
        );
    }

    #[test]
    fn insolation_map_exports_month_calendar_and_derived_provenance() {
        let mut world = World::empty(PathBuf::from("solar-test"));
        world.width = 48;
        world.height = 24;
        world.calendar.month_days[0] = 40.0;
        crate::datasets::DatasetRegistry::default()
            .activate(&mut world, 2)
            .unwrap();
        assert!(Theme::MonthlyInsolation.available(&world));
        let art = Cartography::new(&world);
        let image = atlas_image(&world, &art, Theme::MonthlyInsolation);
        assert_eq!(image.size, [48, 24]);
        assert_eq!(art.legend(&world, Theme::MonthlyInsolation).len(), 7);
        let path = std::env::temp_dir().join(format!("aegaea-solar-{}.png", std::process::id()));
        export_atlas_png(&world, &art, Theme::MonthlyInsolation, &path).unwrap();
        let meta: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path.with_extension("json")).unwrap()).unwrap();
        assert_eq!(meta["month"], 2);
        assert_eq!(meta["units"], "W/m²");
        assert_eq!(meta["dataset_id"], crate::planetary::INSOLATION_ID);
        assert_eq!(meta["planetary_calendar"]["month_days"][0], 40.0);
        assert_eq!(meta["provenance"]["kind"], "derived");
        assert!(meta["derived_from"]
            .as_str()
            .unwrap()
            .contains("circular-orbit"));
        std::fs::remove_file(path.with_extension("json")).unwrap();
        std::fs::remove_file(path).unwrap();
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
    }
}
