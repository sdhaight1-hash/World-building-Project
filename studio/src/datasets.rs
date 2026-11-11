//! Unit-aware, time-indexed scientific raster data for independent worlds.
//!
//! Only the selected month's grid is materialized in memory. Persisted values
//! are sparse authored overrides, not per-month full-resolution JSON arrays.
//! Metadata records provenance and explicit dependency IDs, never guesses a
//! physical simulation from a uniform author-created starting value.
use crate::planetary::INSOLATION_ID;
use crate::world::{geo_to_uv, Grid, World};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Celsius,
    MillimetersPerMonth,
    MillimetersPerYear,
    Hectopascals,
    MetersPerSecond,
    PracticalSalinity,
    Fraction,
}

impl Unit {
    pub fn label(self) -> &'static str {
        match self {
            Self::Celsius => "°C",
            Self::MillimetersPerMonth => "mm/month",
            Self::MillimetersPerYear => "mm/year",
            Self::Hectopascals => "hPa",
            Self::MetersPerSecond => "m/s",
            Self::PracticalSalinity => "PSU",
            Self::Fraction => "fraction",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClimateField {
    Temperature,
    Precipitation,
}

impl ClimateField {
    pub const ALL: [Self; 2] = [Self::Temperature, Self::Precipitation];

    pub fn id(self) -> &'static str {
        match self {
            Self::Temperature => "climate.air_temperature",
            Self::Precipitation => "climate.precipitation",
        }
    }

    pub fn annual_id(self) -> &'static str {
        match self {
            Self::Temperature => "climate.annual_temperature",
            Self::Precipitation => "climate.annual_precipitation",
        }
    }

    pub fn annual_operator(self) -> &'static str {
        match self {
            Self::Temperature => "calendar_day_weighted_mean_of_12_monthly_means",
            Self::Precipitation => "sum_of_12_monthly_totals",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Temperature => "Monthly air temperature",
            Self::Precipitation => "Monthly precipitation",
        }
    }

    pub fn unit(self) -> Unit {
        match self {
            Self::Temperature => Unit::Celsius,
            Self::Precipitation => Unit::MillimetersPerMonth,
        }
    }

    pub fn default_fill(self) -> f32 {
        match self {
            Self::Temperature => 20.0,
            Self::Precipitation => 100.0,
        }
    }

    pub fn bounds(self) -> [f32; 2] {
        match self {
            Self::Temperature => [-100.0, 65.0],
            Self::Precipitation => [0.0, 10_000.0],
        }
    }
}

pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

pub fn month_name(month: u8) -> &'static str {
    if (1..=12).contains(&month) {
        MONTHS[(month - 1) as usize]
    } else {
        "Invalid month"
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Provenance {
    Authored {
        note: String,
    },
    Imported {
        reference: String,
    },
    Derived {
        operator: String,
        #[serde(default)]
        inputs: Vec<String>,
    },
}

impl Provenance {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Authored { .. } => "Author-specified",
            Self::Imported { .. } => "Imported",
            Self::Derived { .. } => "Derived",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Explicit author-provided baseline. Sparse changes are absolute values.
    pub base: f32,
    #[serde(default)]
    pub overrides: BTreeMap<usize, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dataset {
    pub name: String,
    pub unit: Unit,
    pub valid_range: [f32; 2],
    pub size: [usize; 2],
    pub provenance: Provenance,
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Month 1..=12. Missing months are genuinely absent, not filled or interpolated.
    #[serde(default)]
    pub months: BTreeMap<u8, Snapshot>,
}

impl Dataset {
    fn valid_value(&self, v: f32) -> bool {
        v.is_finite() && v >= self.valid_range[0] && v <= self.valid_range[1]
    }

    fn materialize(&self, month: u8) -> Option<Grid> {
        let snapshot = self.months.get(&month)?;
        let mut grid = Grid::new(self.size[0], self.size[1], snapshot.base);
        for (&i, &v) in &snapshot.overrides {
            grid.cells[i] = v;
        }
        Some(grid)
    }
}

/// Unknown future datasets may be stored with their own units and dependencies.
/// The UI currently exposes the two validated climate quantities above.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DatasetRegistry {
    pub fields: BTreeMap<String, Dataset>,
}

impl DatasetRegistry {
    fn valid_id(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 80
            && id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
    }

    /// Dependency graph is validated as one transaction before activation:
    /// unknown IDs, cycles, incompatible grid sizes and inconsistent known
    /// physical quantities never yield partly activated world state.
    pub fn validate(&self, size: [usize; 2]) -> Result<(), String> {
        for (id, field) in &self.fields {
            if !Self::valid_id(id) {
                return Err(format!("Invalid dataset ID: {id}"));
            }
            if id == INSOLATION_ID || ClimateField::ALL.iter().any(|kind| id == kind.annual_id()) {
                return Err(format!(
                    "Dataset ID '{id}' is reserved for computed annual climate maps"
                ));
            }
            if field.name.trim().is_empty() || field.size != size {
                return Err(format!(
                    "Dataset '{id}' has missing name or incorrect grid size"
                ));
            }
            let [low, high] = field.valid_range;
            if !low.is_finite() || !high.is_finite() || low >= high {
                return Err(format!("Dataset '{id}' has invalid numeric limits"));
            }
            for kind in ClimateField::ALL {
                if id == kind.id()
                    && (field.unit != kind.unit() || field.valid_range != kind.bounds())
                {
                    return Err(format!("Dataset '{id}' has incompatible units or limits"));
                }
            }
            for (&month, snapshot) in &field.months {
                if !(1..=12).contains(&month) || !field.valid_value(snapshot.base) {
                    return Err(format!("Invalid baseline/month in dataset '{id}'"));
                }
                for (&index, &v) in &snapshot.overrides {
                    if index >= size[0] * size[1] || !field.valid_value(v) {
                        return Err(format!(
                            "Invalid cell {index} in dataset '{id}', month {month}"
                        ));
                    }
                }
            }
            for parent in &field.dependencies {
                if !self.fields.contains_key(parent) {
                    return Err(format!(
                        "Dataset '{id}' requires missing dataset '{parent}'"
                    ));
                }
            }
        }

        fn visit<'a>(
            id: &'a str,
            fields: &'a BTreeMap<String, Dataset>,
            visiting: &mut BTreeSet<&'a str>,
            visited: &mut BTreeSet<&'a str>,
        ) -> Result<(), String> {
            if visited.contains(id) {
                return Ok(());
            }
            if !visiting.insert(id) {
                return Err(format!("Dataset dependency cycle through '{id}'"));
            }
            for dependency in &fields[id].dependencies {
                visit(dependency, fields, visiting, visited)?;
            }
            visiting.remove(id);
            visited.insert(id);
            Ok(())
        }
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        for id in self.fields.keys() {
            visit(id, &self.fields, &mut visiting, &mut visited)?;
        }
        Ok(())
    }

    /// Register one independent monthly field explicitly. No rainfall or
    /// climate physics is inferred merely by creating its numeric baseline.
    pub fn create_month(
        &mut self,
        world: &mut World,
        kind: ClimateField,
        month: u8,
        fill: f32,
    ) -> Result<(), String> {
        if !(1..=12).contains(&month)
            || !fill.is_finite()
            || fill < kind.bounds()[0]
            || fill > kind.bounds()[1]
        {
            return Err("Invalid month or numeric climate baseline".into());
        }
        let id = kind.id();
        let dataset = self.fields.entry(id.into()).or_insert_with(|| Dataset {
            name: kind.name().into(),
            unit: kind.unit(),
            valid_range: kind.bounds(),
            size: [world.width, world.height],
            provenance: Provenance::Authored {
                note: "Uniform initial author assumption; not computed climate".into(),
            },
            dependencies: vec![],
            months: BTreeMap::new(),
        });
        if dataset.size != [world.width, world.height]
            || dataset.unit != kind.unit()
            || dataset.valid_range != kind.bounds()
        {
            return Err("Existing dataset metadata conflicts with this field".into());
        }
        if dataset.months.contains_key(&month) {
            return Err(format!(
                "{} already exists for {}",
                kind.name(),
                month_name(month)
            ));
        }
        dataset.months.insert(
            month,
            Snapshot {
                base: fill,
                overrides: BTreeMap::new(),
            },
        );
        self.activate(world, month)
    }

    /// Explicit copy, never automatic temporal extrapolation or interpolation.
    pub fn copy_month(
        &mut self,
        world: &mut World,
        kind: ClimateField,
        from: u8,
        to: u8,
    ) -> Result<(), String> {
        if !(1..=12).contains(&from) || !(1..=12).contains(&to) {
            return Err("Month must be 1–12".into());
        }
        let dataset = self
            .fields
            .get_mut(kind.id())
            .ok_or("Source dataset does not exist")?;
        if dataset.months.contains_key(&to) {
            return Err("Target month already exists; refusing overwrite".into());
        }
        let source = dataset
            .months
            .get(&from)
            .ok_or("Source month is missing")?
            .clone();
        dataset.months.insert(to, source);
        self.activate(world, to)
    }

    /// Annual temperature weights monthly means by explicitly declared solar
    /// days; precipitation remains a sum of monthly totals.
    pub fn annual(&self, kind: ClimateField, month_days: &[f64; 12]) -> Option<Grid> {
        let source = self.fields.get(kind.id())?;
        if !(1..=12).all(|month| source.months.contains_key(&month)) {
            return None;
        }
        let year_days: f64 = month_days.iter().sum();
        let weight = |month: usize| -> f32 {
            match kind {
                ClimateField::Temperature => (month_days[month - 1] / year_days) as f32,
                ClimateField::Precipitation => 1.0,
            }
        };
        let baseline: f32 = (1..=12)
            .map(|month| source.months[&(month as u8)].base * weight(month))
            .sum();
        let mut grid = Grid::new(source.size[0], source.size[1], baseline);
        for month in 1..=12 {
            let snapshot = &source.months[&(month as u8)];
            for (&index, &value) in &snapshot.overrides {
                grid.cells[index] += (value - snapshot.base) * weight(month);
            }
        }
        Some(grid)
    }

    pub fn has_month(&self, kind: ClimateField, month: u8) -> bool {
        self.fields
            .get(kind.id())
            .is_some_and(|d| d.months.contains_key(&month))
    }

    pub fn activate(&self, world: &mut World, month: u8) -> Result<(), String> {
        if !(1..=12).contains(&month) {
            return Err("Month must be 1–12".into());
        }
        self.validate([world.width, world.height])?;
        world.calendar.validate()?;
        let mut next: BTreeMap<String, Grid> = self
            .fields
            .iter()
            .filter_map(|(id, dataset)| dataset.materialize(month).map(|g| (id.clone(), g)))
            .collect();
        let mut provenance: BTreeMap<String, Provenance> = self
            .fields
            .iter()
            .filter(|(id, _)| next.contains_key(*id))
            .map(|(id, data)| (id.clone(), data.provenance.clone()))
            .collect();
        for kind in ClimateField::ALL {
            if let Some(grid) = self.annual(kind, &world.calendar.month_days) {
                next.insert(kind.annual_id().into(), grid);
                provenance.insert(
                    kind.annual_id().into(),
                    Provenance::Derived {
                        operator: kind.annual_operator().into(),
                        inputs: if kind == ClimateField::Temperature {
                            vec![kind.id().into(), "planet.calendar".into()]
                        } else {
                            vec![kind.id().into()]
                        },
                    },
                );
            }
        }
        next.insert(
            INSOLATION_ID.into(),
            world
                .calendar
                .monthly_insolation(world.width, world.height, month)?,
        );
        provenance.insert(
            INSOLATION_ID.into(),
            Provenance::Derived {
                operator: "circular_orbit_monthly_mean_daily_toa_irradiance_v1".into(),
                inputs: vec!["planet.calendar".into()],
            },
        );
        world.scientific_fields = next;
        world.scientific_provenance = provenance;
        world.active_month = month;
        Ok(())
    }

    pub fn paint(
        &mut self,
        world: &mut World,
        kind: ClimateField,
        month: u8,
        longitude: f64,
        latitude: f64,
        radius: u32,
        value: f32,
    ) -> Result<DatasetStroke, String> {
        if world.active_month != month {
            return Err("Activate the requested month before painting".into());
        }
        let dataset = self
            .fields
            .get_mut(kind.id())
            .ok_or("Initialize this climate dataset")?;
        if !dataset.valid_value(value) {
            return Err("Paint value outside this field's unit-aware limits".into());
        }
        let snapshot = dataset
            .months
            .get_mut(&month)
            .ok_or("Initialize this month first")?;
        let grid = world
            .scientific_fields
            .get_mut(kind.id())
            .ok_or("Climate grid is unavailable")?;
        let radius = radius.clamp(1, 40) as isize;
        let (u, v) = geo_to_uv(longitude, latitude);
        let cx = (u * grid.width as f64).floor() as isize;
        let cy = ((v * grid.height as f64).floor() as isize).min(grid.height as isize - 1);
        let mut stroke = DatasetStroke {
            id: kind.id().into(),
            month,
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
                stroke.changes.insert(
                    index,
                    DatasetChange {
                        before,
                        after: value,
                        previous_override: snapshot.overrides.get(&index).copied(),
                    },
                );
                grid.cells[index] = value;
                snapshot.overrides.insert(index, value);
            }
        }
        Ok(stroke)
    }
}

#[derive(Debug, Clone)]
struct DatasetChange {
    before: f32,
    after: f32,
    previous_override: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct DatasetStroke {
    id: String,
    month: u8,
    changes: BTreeMap<usize, DatasetChange>,
}

impl DatasetStroke {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn merge(&mut self, other: Self) {
        if other.is_empty() {
            return;
        }
        if self.changes.is_empty() {
            self.id = other.id.clone();
            self.month = other.month;
        }
        if self.id != other.id || self.month != other.month {
            return;
        }
        for (index, change) in other.changes {
            self.changes
                .entry(index)
                .and_modify(|c| c.after = change.after)
                .or_insert(change);
        }
    }

    pub fn undo(&self, world: &mut World, registry: &mut DatasetRegistry) {
        self.restore(world, registry, false);
    }

    pub fn redo(&self, world: &mut World, registry: &mut DatasetRegistry) {
        self.restore(world, registry, true);
    }

    fn restore(&self, world: &mut World, registry: &mut DatasetRegistry, redo: bool) {
        if world.active_month != self.month {
            return;
        }
        let Some(grid) = world.scientific_fields.get_mut(&self.id) else {
            return;
        };
        let Some(snapshot) = registry
            .fields
            .get_mut(&self.id)
            .and_then(|field| field.months.get_mut(&self.month))
        else {
            return;
        };
        for (&index, change) in &self.changes {
            grid.cells[index] = if redo { change.after } else { change.before };
            if redo {
                snapshot.overrides.insert(index, change.after);
            } else if let Some(value) = change.previous_override {
                snapshot.overrides.insert(index, value);
            } else {
                snapshot.overrides.remove(&index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn world() -> World {
        let mut w = World::empty(PathBuf::from("independent"));
        w.width = 48;
        w.height = 24;
        w
    }

    #[test]
    fn annual_climate_requires_all_months_and_respects_monthly_units() {
        let mut w = world();
        let mut registry = DatasetRegistry::default();
        for month in 1..=11u8 {
            registry
                .create_month(&mut w, ClimateField::Temperature, month, month as f32)
                .unwrap();
            registry
                .create_month(
                    &mut w,
                    ClimateField::Precipitation,
                    month,
                    10.0 * month as f32,
                )
                .unwrap();
        }
        assert!(registry
            .annual(ClimateField::Temperature, &w.calendar.month_days)
            .is_none());
        assert!(registry
            .annual(ClimateField::Precipitation, &w.calendar.month_days)
            .is_none());
        assert!(!w
            .scientific_fields
            .contains_key(ClimateField::Temperature.annual_id()));

        registry
            .create_month(&mut w, ClimateField::Temperature, 12, 12.0)
            .unwrap();
        assert!(w
            .scientific_fields
            .contains_key(ClimateField::Temperature.annual_id()));
        assert!(!w
            .scientific_fields
            .contains_key(ClimateField::Precipitation.annual_id()));
        registry
            .create_month(&mut w, ClimateField::Precipitation, 12, 120.0)
            .unwrap();

        let mean = &w.scientific_fields[ClimateField::Temperature.annual_id()];
        let annual_rain = &w.scientific_fields[ClimateField::Precipitation.annual_id()];
        assert!((mean.at(0, 0) - 6.5).abs() < 0.0001);
        assert!((annual_rain.at(0, 0) - 780.0).abs() < 0.001);

        registry.activate(&mut w, 3).unwrap();
        let dab = registry
            .paint(&mut w, ClimateField::Temperature, 3, 0.0, 0.0, 1, 15.0)
            .unwrap();
        assert!(!dab.is_empty());
        registry
            .paint(&mut w, ClimateField::Precipitation, 3, 0.0, 0.0, 1, 300.0)
            .unwrap();
        registry.activate(&mut w, 3).unwrap();
        let new_mean = &w.scientific_fields[ClimateField::Temperature.annual_id()];
        let new_rain = &w.scientific_fields[ClimateField::Precipitation.annual_id()];
        assert!(new_mean.cells.iter().any(|&v| (v - 7.5).abs() < 0.001));
        assert!(new_rain.cells.iter().any(|&v| (v - 1050.0).abs() < 0.01));
        registry.activate(&mut w, 9).unwrap();
        assert!(w.scientific_fields[ClimateField::Temperature.annual_id()]
            .cells
            .iter()
            .any(|&v| (v - 7.5).abs() < 0.001));
        assert_eq!(
            w.scientific_provenance[ClimateField::Precipitation.annual_id()],
            Provenance::Derived {
                operator: "sum_of_12_monthly_totals".into(),
                inputs: vec![ClimateField::Precipitation.id().into()],
            }
        );
    }

    #[test]
    fn annual_climate_is_rebuilt_from_sparse_months_after_json_roundtrip() {
        let mut source = world();
        let mut registry = DatasetRegistry::default();
        for month in 1..=12u8 {
            registry
                .create_month(&mut source, ClimateField::Temperature, month, 16.0)
                .unwrap();
            registry
                .create_month(&mut source, ClimateField::Precipitation, month, 100.0)
                .unwrap();
        }

        registry.activate(&mut source, 4).unwrap();
        registry
            .paint(&mut source, ClimateField::Temperature, 4, 0.0, 0.0, 1, 28.0)
            .unwrap();
        registry
            .paint(
                &mut source,
                ClimateField::Precipitation,
                4,
                0.0,
                0.0,
                1,
                250.0,
            )
            .unwrap();
        registry.activate(&mut source, 4).unwrap();

        let temperature = source.scientific_fields[ClimateField::Temperature.annual_id()]
            .cells
            .clone();
        let precipitation = source.scientific_fields[ClimateField::Precipitation.annual_id()]
            .cells
            .clone();
        assert!(temperature
            .iter()
            .any(|value| (*value - 17.0).abs() < 0.001));
        assert!(precipitation
            .iter()
            .any(|value| (*value - 1350.0).abs() < 0.001));

        let json = serde_json::to_string(&registry).unwrap();
        assert!(!json.contains(ClimateField::Temperature.annual_id()));
        assert!(!json.contains(ClimateField::Precipitation.annual_id()));
        let restored: DatasetRegistry = serde_json::from_str(&json).unwrap();
        let mut reopened = world();
        restored.activate(&mut reopened, 10).unwrap();
        assert_eq!(
            reopened.scientific_fields[ClimateField::Temperature.annual_id()].cells,
            temperature
        );
        assert_eq!(
            reopened.scientific_fields[ClimateField::Precipitation.annual_id()].cells,
            precipitation
        );
    }

    #[test]
    fn annual_derived_ids_cannot_be_overwritten_by_authored_sources() {
        let mut w = world();
        let mut registry = DatasetRegistry::default();
        registry
            .create_month(&mut w, ClimateField::Temperature, 1, 15.0)
            .unwrap();
        let synthetic = registry.fields[ClimateField::Temperature.id()].clone();
        registry
            .fields
            .insert(ClimateField::Temperature.annual_id().into(), synthetic);
        let error = registry.validate([w.width, w.height]).unwrap_err();
        assert!(error.contains("reserved"), "{error}");
    }

    #[test]
    fn calendar_day_weights_annual_temperature_without_changing_precipitation_totals() {
        let mut w = world();
        let mut registry = DatasetRegistry::default();
        for month in 1..=12 {
            registry
                .create_month(
                    &mut w,
                    ClimateField::Temperature,
                    month,
                    if month == 1 { 30.0 } else { 10.0 },
                )
                .unwrap();
            registry
                .create_month(&mut w, ClimateField::Precipitation, month, 100.0)
                .unwrap();
        }
        let uniform = w.scientific_fields[ClimateField::Temperature.annual_id()].at(0, 0);
        assert!((uniform - 11.666_667).abs() < 0.001);
        w.calendar.month_days[0] = 60.0;
        registry.activate(&mut w, 7).unwrap();
        let weighted = w.scientific_fields[ClimateField::Temperature.annual_id()].at(0, 0);
        assert!((weighted - (30.0 * 60.0 + 10.0 * 330.0) / 390.0).abs() < 0.001);
        assert_eq!(w.scientific_fields[ClimateField::Precipitation.annual_id()].at(0, 0), 1200.0);
        assert_eq!(
            w.scientific_provenance[ClimateField::Temperature.annual_id()],
            Provenance::Derived {
                operator: "calendar_day_weighted_mean_of_12_monthly_means".into(),
                inputs: vec![ClimateField::Temperature.id().into(), "planet.calendar".into()],
            }
        );

        let mut impostor = registry.clone();
        impostor.fields.insert(INSOLATION_ID.into(), registry.fields[ClimateField::Temperature.id()].clone());
        assert!(impostor.activate(&mut w, 7).unwrap_err().contains("reserved"));
    }

    #[test]
    fn month_labels_do_not_map_invalid_zero_to_january() {
        assert_eq!(month_name(1), "January");
        assert_eq!(month_name(12), "December");
        assert_eq!(month_name(0), "Invalid month");
        assert_eq!(month_name(13), "Invalid month");
    }

    #[test]
    fn monthly_climate_is_sparse_and_isolated_by_month() {
        let mut w = world();
        let mut registry = DatasetRegistry::default();
        registry
            .create_month(&mut w, ClimateField::Temperature, 1, 18.0)
            .unwrap();
        registry
            .create_month(&mut w, ClimateField::Temperature, 7, 27.0)
            .unwrap();
        registry.activate(&mut w, 1).unwrap();
        let mut brush = DatasetStroke::default();
        brush.merge(
            registry
                .paint(&mut w, ClimateField::Temperature, 1, 179.9, 0.0, 2, 42.0)
                .unwrap(),
        );
        assert!(!brush.is_empty());
        assert!(w.scientific_fields[ClimateField::Temperature.id()]
            .cells
            .iter()
            .any(|&v| v == 42.0));
        brush.undo(&mut w, &mut registry);
        assert!(w.scientific_fields[ClimateField::Temperature.id()]
            .cells
            .iter()
            .all(|&v| v == 18.0));
        brush.redo(&mut w, &mut registry);
        let january = w.scientific_fields[ClimateField::Temperature.id()]
            .cells
            .clone();
        registry.activate(&mut w, 7).unwrap();
        assert!(w.scientific_fields[ClimateField::Temperature.id()]
            .cells
            .iter()
            .all(|&v| v == 27.0));
        registry.activate(&mut w, 1).unwrap();
        assert_eq!(
            w.scientific_fields[ClimateField::Temperature.id()].cells,
            january
        );
        assert!(
            registry.fields[ClimateField::Temperature.id()].months[&1]
                .overrides
                .len()
                > 0
        );
    }

    #[test]
    fn copy_month_is_explicit_and_will_not_overwrite_existing_month() {
        let mut w = world();
        let mut registry = DatasetRegistry::default();
        registry
            .create_month(&mut w, ClimateField::Precipitation, 3, 100.0)
            .unwrap();
        registry
            .paint(&mut w, ClimateField::Precipitation, 3, 0.0, 0.0, 2, 300.0)
            .unwrap();
        registry
            .copy_month(&mut w, ClimateField::Precipitation, 3, 4)
            .unwrap();
        assert_eq!(w.active_month, 4);
        assert!(w.scientific_fields[ClimateField::Precipitation.id()]
            .cells
            .iter()
            .any(|&v| v == 300.0));
        assert!(registry
            .copy_month(&mut w, ClimateField::Precipitation, 3, 4)
            .is_err());
        registry.activate(&mut w, 5).unwrap();
        assert!(!w
            .scientific_fields
            .contains_key(ClimateField::Precipitation.id()));
        assert!(w.scientific_fields.contains_key(INSOLATION_ID));
    }

    #[test]
    fn project_json_round_trip_and_invalid_grid_rejection() {
        let mut w = world();
        let mut reg = DatasetRegistry::default();
        reg.create_month(&mut w, ClimateField::Temperature, 1, -10.0)
            .unwrap();
        reg.create_month(&mut w, ClimateField::Precipitation, 1, 80.0)
            .unwrap();
        let bytes = serde_json::to_vec(&reg).unwrap();
        let restored: DatasetRegistry = serde_json::from_slice(&bytes).unwrap();
        let mut other = world();
        restored.activate(&mut other, 1).unwrap();
        assert_eq!(other.scientific_fields.len(), 3);
        assert_eq!(
            other.scientific_fields[ClimateField::Precipitation.id()].at(0, 0),
            80.0
        );

        let mut wrong = restored.clone();
        wrong
            .fields
            .get_mut(ClimateField::Temperature.id())
            .unwrap()
            .size = [49, 24];
        assert!(wrong.activate(&mut other, 1).is_err());
        assert_eq!(
            other.scientific_fields.len(),
            3,
            "failed activation is transactional"
        );
        let mut corrupt = restored.clone();
        corrupt
            .fields
            .get_mut(ClimateField::Temperature.id())
            .unwrap()
            .months
            .get_mut(&1)
            .unwrap()
            .overrides
            .insert(48 * 24, 20.0);
        assert!(corrupt.activate(&mut other, 1).is_err());
        assert!(reg
            .paint(&mut w, ClimateField::Precipitation, 1, 0.0, 0.0, 2, -1.0)
            .is_err());
    }

    #[test]
    fn dependency_dag_rejects_missing_inputs_and_cycles() {
        let mut w = world();
        let mut r = DatasetRegistry::default();
        r.create_month(&mut w, ClimateField::Temperature, 1, 15.0)
            .unwrap();
        r.create_month(&mut w, ClimateField::Precipitation, 1, 120.0)
            .unwrap();
        r.fields
            .get_mut(ClimateField::Temperature.id())
            .unwrap()
            .dependencies = vec![ClimateField::Precipitation.id().into()];
        r.validate([48, 24]).unwrap();
        r.fields
            .get_mut(ClimateField::Precipitation.id())
            .unwrap()
            .dependencies = vec![ClimateField::Temperature.id().into()];
        assert!(r.validate([48, 24]).unwrap_err().contains("cycle"));
        r.fields
            .get_mut(ClimateField::Precipitation.id())
            .unwrap()
            .dependencies = vec!["missing.flux".into()];
        assert!(r.validate([48, 24]).unwrap_err().contains("missing"));
    }
}
