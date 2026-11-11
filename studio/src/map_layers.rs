//! Explicit project-local authoring of scientific raster inputs.
//! A uniform fill is an author assumption, NEVER a computed geological model.
//! Imported TIFFs remain immutable; this is a sparse JSON overlay.
use crate::world::{geo_to_uv, CratonInfo, Grid, PlateInfo, ProvinceInfo, World};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerKind {
    Plates,
    Cratons,
    Provinces,
    Crust,
    Lithosphere,
    HeatFlow,
}

impl LayerKind {
    pub const ALL: [Self; 6] = [
        Self::Plates,
        Self::Cratons,
        Self::Provinces,
        Self::Crust,
        Self::Lithosphere,
        Self::HeatFlow,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Plates => "Plate IDs",
            Self::Cratons => "Craton IDs",
            Self::Provinces => "Crustal province IDs",
            Self::Crust => "Crust thickness",
            Self::Lithosphere => "Lithosphere depth",
            Self::HeatFlow => "Surface heat flow",
        }
    }

    pub fn units(self) -> &'static str {
        match self {
            Self::Plates | Self::Cratons | Self::Provinces => "category ID",
            Self::Crust | Self::Lithosphere => "km",
            Self::HeatFlow => "mW/m²",
        }
    }

    pub fn default_fill(self) -> f32 {
        match self {
            Self::Plates => 1.0,
            Self::Cratons | Self::Provinces => 0.0,
            Self::Crust => 7.0,
            Self::Lithosphere => 100.0,
            Self::HeatFlow => 60.0,
        }
    }

    pub fn valid_value(self, value: f32) -> bool {
        if !value.is_finite() {
            return false;
        }
        match self {
            Self::Plates | Self::Cratons => value.fract() == 0.0 && (0.0..=255.0).contains(&value),
            Self::Provinces => value.fract() == 0.0 && (0.0..=10.0).contains(&value),
            Self::Crust => (0.0..=100.0).contains(&value),
            Self::Lithosphere => (0.0..=450.0).contains(&value),
            Self::HeatFlow => (0.0..=500.0).contains(&value),
        }
    }

    pub fn is_categorical(self) -> bool {
        matches!(self, Self::Plates | Self::Cratons | Self::Provinces)
    }

    pub fn field(self, world: &World) -> &Option<Grid> {
        match self {
            Self::Plates => &world.plate_ids,
            Self::Cratons => &world.craton_ids,
            Self::Provinces => &world.province_ids,
            Self::Crust => &world.crust_km,
            Self::Lithosphere => &world.lithosphere_km,
            Self::HeatFlow => &world.heatflow,
        }
    }

    pub fn field_mut(self, world: &mut World) -> &mut Option<Grid> {
        match self {
            Self::Plates => &mut world.plate_ids,
            Self::Cratons => &mut world.craton_ids,
            Self::Provinces => &mut world.province_ids,
            Self::Crust => &mut world.crust_km,
            Self::Lithosphere => &mut world.lithosphere_km,
            Self::HeatFlow => &mut world.heatflow,
        }
    }
}

const PROVINCE_CLASSES: [&str; 11] = [
    "ocean_basin",
    "ocean_spreading_axis",
    "ocean_trench",
    "noncratonic_continent",
    "exposed_cratonic_shield",
    "sedimentary_cratonic_platform",
    "ancient_mobile_belt",
    "collision_orogen",
    "volcanic_arc",
    "continental_rift",
    "transform_shear_zone",
];

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AuthoredGrid {
    /// Some(value) creates a new source layer. None overlays a pre-existing TIFF.
    pub base_fill: Option<f32>,
    pub size: [usize; 2],
    #[serde(default)]
    pub overrides: BTreeMap<usize, f32>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectLayers {
    pub grids: BTreeMap<LayerKind, AuthoredGrid>,
    pub plates: Option<Vec<PlateInfo>>,
    pub cratons: Option<Vec<CratonInfo>>,
    pub provinces: Option<Vec<ProvinceInfo>>,
}

impl ProjectLayers {
    pub fn create(&mut self, world: &mut World, kind: LayerKind, fill: f32) -> Result<(), String> {
        if !kind.valid_value(fill) {
            return Err(format!("Invalid {} fill: {fill}", kind.name()));
        }
        if self.grids.contains_key(&kind) {
            return Err(format!("{} already has an authored layer", kind.name()));
        }
        let has_source = kind.field(world).is_some();
        if !has_source {
            *kind.field_mut(world) = Some(Grid::new(world.width, world.height, fill));
        }
        self.grids.insert(
            kind,
            AuthoredGrid {
                base_fill: if has_source { None } else { Some(fill) },
                size: [world.width, world.height],
                overrides: BTreeMap::new(),
            },
        );
        match kind {
            LayerKind::Plates => {
                let catalog = self.plates.get_or_insert_with(|| world.plates.clone());
                if catalog.is_empty() {
                    // This is a labeled editable starting category, not a tectonic solution.
                    catalog.push(PlateInfo {
                        plate_id: 1,
                        plate: "Plate 1".into(),
                        color_rgb: "91,151,184".into(),
                        euler_pole_lon_deg: None,
                        euler_pole_lat_deg: None,
                        signed_rate_deg_per_ma: None,
                    });
                }
                world.plates.clone_from(catalog);
            }
            LayerKind::Cratons => {
                self.cratons.get_or_insert_with(|| world.cratons.clone());
            }
            LayerKind::Provinces => {
                let catalog = self.provinces.get_or_insert_with(|| world.provinces.clone());
                if catalog.is_empty() {
                    catalog.extend(PROVINCE_CLASSES.iter().enumerate().map(|(id, &name)| ProvinceInfo {
                        province_id: id as u8,
                        name: name.into(),
                    }));
                }
                world.provinces.clone_from(catalog);
            }
            _ => {}
        }
        Ok(())
    }

    /// Reconstitute authored data from an independent project, or apply a
    /// non-destructive overlay to source rasters. Invalid grids fail explicitly.
    pub fn apply(&self, world: &mut World) -> Result<(), String> {
        for (kind, authored) in &self.grids {
            if authored.size != [world.width, world.height] {
                return Err(format!("{} grid size differs from project dataset", kind.name()));
            }
            if let Some(fill) = authored.base_fill {
                if !kind.valid_value(fill) {
                    return Err(format!("Invalid {} base fill", kind.name()));
                }
            } else if kind.field(world).is_none() {
                return Err(format!("{} overlay has no source raster", kind.name()));
            }
            for (&i, &value) in &authored.overrides {
                if i >= world.width * world.height || !kind.valid_value(value) {
                    return Err(format!("Invalid {} value at cell {i}", kind.name()));
                }
            }
        }
        for (kind, authored) in &self.grids {
            if let Some(fill) = authored.base_fill {
                *kind.field_mut(world) = Some(Grid::new(world.width, world.height, fill));
            }
            let grid = kind.field_mut(world).as_mut().unwrap();
            for (&index, &value) in &authored.overrides {
                grid.cells[index] = value;
            }
        }
        if let Some(plates) = &self.plates {
            world.plates.clone_from(plates);
        }
        if let Some(cratons) = &self.cratons {
            world.cratons.clone_from(cratons);
        }
        if let Some(provinces) = &self.provinces {
            world.provinces.clone_from(provinces);
        }
        Ok(())
    }

    pub fn paint(
        &mut self,
        world: &mut World,
        kind: LayerKind,
        longitude: f64,
        latitude: f64,
        radius: u32,
        value: f32,
    ) -> Result<LayerStroke, String> {
        if !kind.valid_value(value) {
            return Err(format!("Invalid {} paint value: {value}", kind.name()));
        }
        if kind == LayerKind::Plates
            && !world.plates.iter().any(|p| p.plate_id as f32 == value)
        {
            return Err("Create a plate record before painting its ID".into());
        }
        if kind == LayerKind::Cratons && value != 0.0
            && !world.cratons.iter().any(|c| c.craton_id as f32 == value)
        {
            return Err("Create a craton record before painting its ID".into());
        }
        if kind == LayerKind::Provinces
            && !world.provinces.iter().any(|p| p.province_id as f32 == value)
        {
            return Err("Create a province record before painting its ID".into());
        }
        let authored = self.grids.get_mut(&kind).ok_or("Initialize the layer before painting")?;
        let grid = kind.field_mut(world).as_mut().ok_or("Layer source unavailable")?;
        let (u, v) = geo_to_uv(longitude, latitude);
        let cx = (u * grid.width as f64).floor() as isize;
        let cy = ((v * grid.height as f64).floor() as isize).min(grid.height as isize - 1);
        let radius = radius.clamp(1, 40) as isize;
        let mut stroke = LayerStroke {
            kind: Some(kind),
            changes: BTreeMap::new(),
        };
        for oy in -radius..=radius {
            let y = cy + oy;
            if y < 0 || y >= grid.height as isize {
                continue;
            }
            for ox in -radius..=radius {
                if ox * ox + oy * oy > radius * radius {
                    continue;
                }
                let x = (cx + ox).rem_euclid(grid.width as isize) as usize;
                let index = y as usize * grid.width + x;
                let before = grid.cells[index];
                if before == value {
                    continue;
                }
                stroke.changes.insert(index, LayerChange {
                    before,
                    after: value,
                    previous_override: authored.overrides.get(&index).copied(),
                });
                grid.cells[index] = value;
                authored.overrides.insert(index, value);
            }
        }
        Ok(stroke)
    }

    pub fn add_plate(&mut self, world: &mut World) -> Result<u8, String> {
        if !self.grids.contains_key(&LayerKind::Plates) {
            return Err("Create the plate layer first".into());
        }
        let catalog = self.plates.get_or_insert_with(|| world.plates.clone());
        let id = (1..=255u8).find(|id| !catalog.iter().any(|p| p.plate_id == *id))
            .ok_or("All 255 plate IDs are in use")?;
        catalog.push(PlateInfo {
            plate_id: id,
            plate: format!("Plate {id}"),
            color_rgb: format!("{},{},{}", 70 + id as u16 * 23 % 150, 80 + id as u16 * 39 % 145, 90 + id as u16 * 31 % 145),
            euler_pole_lon_deg: None,
            euler_pole_lat_deg: None,
            signed_rate_deg_per_ma: None,
        });
        world.plates.clone_from(catalog);
        Ok(id)
    }

    pub fn add_craton(&mut self, world: &mut World) -> Result<u8, String> {
        if !self.grids.contains_key(&LayerKind::Cratons) {
            return Err("Create the craton layer first".into());
        }
        let parent = world.plates.first().ok_or("Create a plate before adding cratons")?.plate.clone();
        let catalog = self.cratons.get_or_insert_with(|| world.cratons.clone());
        let id = (1..=255u8).find(|id| !catalog.iter().any(|c| c.craton_id == *id))
            .ok_or("All 255 craton IDs are in use")?;
        catalog.push(CratonInfo {
            craton_id: id,
            name: format!("Craton {id}"),
            plate: parent,
            basement_age_ga: 3.0,
        });
        world.cratons.clone_from(catalog);
        Ok(id)
    }
}

#[derive(Debug, Clone)]
pub struct LayerChange {
    before: f32,
    after: f32,
    previous_override: Option<f32>,
}

#[derive(Debug, Default, Clone)]
pub struct LayerStroke {
    pub kind: Option<LayerKind>,
    changes: BTreeMap<usize, LayerChange>,
}

impl LayerStroke {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn merge(&mut self, dab: Self) {
        if dab.kind.is_none() {
            return;
        }
        if self.kind.is_none() {
            self.kind = dab.kind;
        }
        if self.kind != dab.kind {
            return;
        }
        for (index, change) in dab.changes {
            self.changes.entry(index)
                .and_modify(|existing| existing.after = change.after)
                .or_insert(change);
        }
    }

    pub fn undo(&self, world: &mut World, layers: &mut ProjectLayers) {
        self.restore(world, layers, false);
    }

    pub fn redo(&self, world: &mut World, layers: &mut ProjectLayers) {
        self.restore(world, layers, true);
    }

    fn restore(&self, world: &mut World, layers: &mut ProjectLayers, redo: bool) {
        let Some(kind) = self.kind else { return };
        let Some(authored) = layers.grids.get_mut(&kind) else { return };
        let Some(grid) = kind.field_mut(world).as_mut() else { return };
        for (&index, change) in &self.changes {
            grid.cells[index] = if redo { change.after } else { change.before };
            if redo {
                authored.overrides.insert(index, change.after);
            } else if let Some(previous) = change.previous_override {
                authored.overrides.insert(index, previous);
            } else {
                authored.overrides.remove(&index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn authoring_all_six_sources_and_reload() {
        let mut world = World::empty(PathBuf::from("independent"));
        let mut layers = ProjectLayers::default();
        for kind in LayerKind::ALL {
            layers.create(&mut world, kind, kind.default_fill()).unwrap();
        }
        assert_eq!(world.plates.len(), 1);
        assert_eq!(world.provinces.len(), 11);
        assert!(world.plate_motion.is_none());
        let second = layers.add_plate(&mut world).unwrap();
        assert_eq!(second, 2);
        let craton = layers.add_craton(&mut world).unwrap();
        assert_eq!(craton, 1);
        let stroke = layers.paint(&mut world, LayerKind::Plates, 179.9, 0.0, 3, 2.0).unwrap();
        assert!(!stroke.is_empty());
        let original = world.plate_ids.as_ref().unwrap().cells.clone();
        stroke.undo(&mut world, &mut layers);
        assert!(world.plate_ids.as_ref().unwrap().cells.iter().all(|&p| p == 1.0));
        stroke.redo(&mut world, &mut layers);
        assert_eq!(world.plate_ids.as_ref().unwrap().cells, original);
        layers.paint(&mut world, LayerKind::Cratons, 179.9, 0.0, 2, 1.0).unwrap();
        layers.paint(&mut world, LayerKind::Provinces, 30.0, -30.0, 2, 7.0).unwrap();
        layers.paint(&mut world, LayerKind::Crust, 30.0, -30.0, 2, 42.0).unwrap();
        layers.paint(&mut world, LayerKind::Lithosphere, 30.0, -30.0, 2, 180.0).unwrap();
        layers.paint(&mut world, LayerKind::HeatFlow, 30.0, -30.0, 2, 90.0).unwrap();
        let json = serde_json::to_string(&layers).unwrap();
        let restored: ProjectLayers = serde_json::from_str(&json).unwrap();
        let mut reloaded = World::empty(PathBuf::from("independent"));
        restored.apply(&mut reloaded).unwrap();
        for kind in LayerKind::ALL {
            assert_eq!(kind.field(&reloaded).as_ref().unwrap().cells,
                       kind.field(&world).as_ref().unwrap().cells);
        }
        assert_eq!(reloaded.plates.len(), 2);
        assert_eq!(reloaded.cratons.len(), 1);
    }

    #[test]
    fn rejects_invalid_cells_and_never_rewrites_imported_raster() {
        let mut world = World::empty(PathBuf::from("imported"));
        world.crust_km = Some(Grid::new(1024, 512, 35.0));
        let mut layers = ProjectLayers::default();
        layers.create(&mut world, LayerKind::Crust, 7.0).unwrap();
        assert_eq!(layers.grids[&LayerKind::Crust].base_fill, None);
        layers.paint(&mut world, LayerKind::Crust, 0.0, 0.0, 1, 40.0).unwrap();
        assert!(layers.paint(&mut world, LayerKind::Crust, 0.0, 0.0, 1, -4.0).is_err());
        let mut source = World::empty(PathBuf::new());
        source.crust_km = Some(Grid::new(1024, 512, 35.0));
        layers.apply(&mut source).unwrap();
        assert_eq!(source.crust_km.as_ref().unwrap().cells, world.crust_km.as_ref().unwrap().cells);
        let mut no_source = World::empty(PathBuf::new());
        assert!(layers.apply(&mut no_source).is_err());
    }
}
