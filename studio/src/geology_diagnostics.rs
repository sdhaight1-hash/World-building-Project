//! Non-destructive consistency diagnostics across fictional geological map layers.
//!
//! Inspired by the practical need to keep authored plate/crust/terrain inputs
//! coherent. A warning is evidence of inconsistent *source classifications*,
//! not proof that a geological or tectonic scenario is physically impossible.
//! Never modify maps or fabricate corrected categories in this module.
use crate::world::{Grid, World};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path};

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct Issue {
    pub code: String,
    pub severity: Severity,
    pub description: String,
    pub affected_cells: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Counts {
    pub total_cells: usize,
    pub plate_cells_checked: usize,
    pub craton_cells_checked: usize,
    pub province_cells_checked: usize,
    pub unknown_plate_cells: usize,
    pub unknown_craton_cells: usize,
    pub unknown_province_cells: usize,
    pub conflicting_craton_plate_cells: usize,
    pub unresolved_craton_plate_metadata: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub method: String,
    pub grid_size: [usize; 2],
    pub counts: Counts,
    pub issues: Vec<Issue>,
}

impl Report {
    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| matches!(i.severity,Severity::Warning | Severity::Error))
            .count()
    }

    pub fn write(&self, path:&Path) -> Result<(), String> {
        if let Some(parent)=path.parent() {
            std::fs::create_dir_all(parent).map_err(|e|e.to_string())?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(self).map_err(|e|e.to_string())?)
            .map_err(|e|format!("{}: {e}",path.display()))
    }
}

/// Category rasters must contain exact integer identifiers; NaN/negative
/// values must never silently collapse to ID 0 during a float-to-int cast.
fn category(value: f32) -> Option<usize> {
    if value.is_finite() && (0.0..=255.0).contains(&value) && value.fract()==0.0 {
        Some(value as usize)
    } else {
        None
    }
}

/// All layers have the same native dimensions because World::load validates
/// alignment. Still guard direct/synthetic callers from inconsistent grids.
fn valid_grid(world: &World, grid: &Grid) -> bool {
    grid.width==world.width
        && grid.height==world.height
        && grid.cells.len()==world.width*world.height
}

pub fn examine(world: &World) -> Report {
    let mut report=Report {
        method:"aegaea-source-category-integrity-v1 (cell-count; not area-weighted)".into(),
        grid_size:[world.width,world.height],
        counts:Counts {total_cells:world.width*world.height,..Counts::default()},
        issues:Vec::new(),
    };
    if world.plate_ids.is_none() && world.craton_ids.is_none()
        && world.province_ids.is_none() {
        report.issues.push(Issue {
            code:"no_geological_layers".into(),
            severity:Severity::Info,
            description:"No plate, craton or province category rasters loaded; blank worlds are valid".into(),
            affected_cells:0,
        });
        return report;
    }

    for (label,grid) in [
        ("plate",&world.plate_ids),
        ("craton",&world.craton_ids),
        ("province",&world.province_ids)
    ] {
        if let Some(grid)=grid {
            if !valid_grid(world,grid) {
                report.issues.push(Issue {
                    code:format!("invalid_{label}_grid"),
                    severity:Severity::Error,
                    description:format!("{label} category grid is not aligned to the world grid"),
                    affected_cells:grid.cells.len(),
                });
                return report;
            }
        }
    }

    let mut known_plates=[false;256];
    let mut known_cratons=[false;256];
    let mut known_provinces=[false;256];
    let mut plate_by_name=BTreeMap::new();
    let mut expected_plate:[Option<usize>;256]=[None;256];

    for plate in &world.plates {
        known_plates[plate.plate_id as usize]=true;
        plate_by_name.insert(plate.plate.as_str(),plate.plate_id as usize);
    }
    for craton in &world.cratons {
        known_cratons[craton.craton_id as usize]=true;
        if let Some(&plate_id)=plate_by_name.get(craton.plate.as_str()) {
            expected_plate[craton.craton_id as usize]=Some(plate_id);
        } else {
            report.counts.unresolved_craton_plate_metadata +=1;
        }
    }
    for province in &world.provinces {
        known_provinces[province.province_id as usize]=true;
    }

    for index in 0..world.width*world.height {
        let plate=world.plate_ids.as_ref().and_then(|g|category(g.cells[index]));
        let craton=world.craton_ids.as_ref().and_then(|g|category(g.cells[index]));
        let province=world.province_ids.as_ref().and_then(|g|category(g.cells[index]));
        if world.plate_ids.is_some() {
            report.counts.plate_cells_checked+=1;
            if plate.filter(|&i|known_plates[i]).is_none() {
                report.counts.unknown_plate_cells+=1;
            }
        }
        if world.craton_ids.is_some() {
            report.counts.craton_cells_checked+=1;
            // Craton 0 is background; it need not have a metadata record.
            if craton != Some(0) && craton.filter(|&i|known_cratons[i]).is_none() {
                report.counts.unknown_craton_cells+=1;
            }
        }
        if world.province_ids.is_some() {
            report.counts.province_cells_checked+=1;
            if province.filter(|&i|known_provinces[i]).is_none() {
                report.counts.unknown_province_cells+=1;
            }
        }
        if let (Some(plate),Some(craton))=(plate,craton) {
            if craton != 0 && known_cratons[craton] {
                if let Some(expected)=expected_plate[craton] {
                    if plate != expected {
                        report.counts.conflicting_craton_plate_cells+=1;
                    }
                }
            }
        }
    }

    for (code,count,description) in [
        ("unknown_plate_ids",report.counts.unknown_plate_cells,
            "Plate raster has cell values with no matching plate metadata or invalid category encoding"),
        ("unknown_craton_ids",report.counts.unknown_craton_cells,
            "Non-background craton IDs are unknown or invalid"),
        ("unknown_province_ids",report.counts.unknown_province_cells,
            "Crustal province raster has class IDs not present in its metadata"),
    ] {
        if count>0 {
            report.issues.push(Issue {
                code:code.into(),severity:Severity::Error,
                description:description.into(),affected_cells:count
            });
        }
    }
    if report.counts.unresolved_craton_plate_metadata>0 {
        report.issues.push(Issue {
            code:"craton_parent_plate_not_found".into(),severity:Severity::Warning,
            description:"Some craton records refer to plate names missing from the plate table".into(),
            affected_cells:report.counts.unresolved_craton_plate_metadata
        });
    }
    if report.counts.conflicting_craton_plate_cells>0 {
        report.issues.push(Issue {
            code:"craton_plate_affiliation_conflicts".into(),severity:Severity::Warning,
            description:"Raster plate IDs differ from parent-plate assignments in craton metadata; inspect tectonic geometry before claiming a coherent reconstruction".into(),
            affected_cells:report.counts.conflicting_craton_plate_cells
        });
    }
    if world.craton_ids.is_some() && world.plate_ids.is_none() {
        report.issues.push(Issue {
            code:"cannot_verify_craton_plate_affiliation".into(),severity:Severity::Info,
            description:"Craton layer exists without a plate grid; parent-plate alignment was not checked".into(),
            affected_cells:0
        });
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{CratonInfo,PlateInfo,ProvinceInfo};
    use std::path::PathBuf;

    fn world() -> World {
        let mut w=World::empty(PathBuf::from("unrelated-world"));
        w.width=4;w.height=3;
        w.plates=vec![
            PlateInfo {plate_id:0,plate:"AAA".into(),color_rgb:"1,2,3".into()},
            PlateInfo {plate_id:1,plate:"BBB".into(),color_rgb:"4,5,6".into()},
        ];
        w.cratons=vec![
            CratonInfo {craton_id:1,name:"Old Core".into(),plate:"AAA".into(),basement_age_ga:3.1}
        ];
        w.provinces=vec![
            ProvinceInfo {province_id:0,name:"ocean".into()},
            ProvinceInfo {province_id:1,name:"continental".into()},
        ];
        w.plate_ids=Some(Grid::new(4,3,0.0));
        w.craton_ids=Some(Grid::new(4,3,0.0));
        w.province_ids=Some(Grid::new(4,3,1.0));
        w
    }

    #[test]
    fn consistent_map_has_no_warnings_and_keeps_sources() {
        let mut w=world();
        w.craton_ids.as_mut().unwrap().cells[5]=1.0;
        let before=w.craton_ids.as_ref().unwrap().cells.clone();
        let result=examine(&w);
        assert_eq!(result.warning_count(),0);
        assert_eq!(result.counts.conflicting_craton_plate_cells,0);
        assert_eq!(before,w.craton_ids.unwrap().cells);
    }

    #[test]
    fn inconsistent_craton_plate_is_reported_but_not_modified() {
        let mut w=world();
        w.craton_ids.as_mut().unwrap().cells[5]=1.0;
        w.plate_ids.as_mut().unwrap().cells[5]=1.0;
        let result=examine(&w);
        assert_eq!(result.counts.conflicting_craton_plate_cells,1);
        assert!(result.issues.iter().any(|i|
            i.code=="craton_plate_affiliation_conflicts"));
        assert_eq!(w.plate_ids.unwrap().cells[5],1.0);
    }

    #[test]
    fn unknown_ids_and_nan_are_not_cast_to_background() {
        let mut w=world();
        w.plate_ids.as_mut().unwrap().cells[0]=f32::NAN;
        w.craton_ids.as_mut().unwrap().cells[1]=99.0;
        w.province_ids.as_mut().unwrap().cells[2]=300.0;
        let report=examine(&w);
        assert_eq!(report.counts.unknown_plate_cells,1);
        assert_eq!(report.counts.unknown_craton_cells,1);
        assert_eq!(report.counts.unknown_province_cells,1);
        assert!(report.warning_count()>=3);
    }

    #[test]
    fn blank_world_is_not_incorrectly_classified_as_broken() {
        let w=World::empty(PathBuf::from("new-world"));
        let report=examine(&w);
        assert_eq!(report.warning_count(),0);
        assert_eq!(report.issues[0].code,"no_geological_layers");
    }

    #[test]
    fn report_is_serializable_and_keeps_scientific_status() {
        let report=examine(&world());
        let output=serde_json::to_string(&report).unwrap();
        assert!(output.contains("cell-count; not area-weighted"));
        assert!(output.contains("total_cells"));
    }
}
