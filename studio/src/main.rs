//! Aegaea Studio P0: a modern wgpu-backed desktop shell around physical-world maps.
//! Purposefully scoped: shared atlas/globe source data, inspector, local point annotation.
mod cartography;
mod datasets;
mod geology_diagnostics;
mod globe3d;
mod map_graph;
mod map_layers;
mod plate_kinematics;
mod publication;
mod rainshadow;
mod render;
mod terrain_edit;
mod terrain_generation;
mod world;

use cartography::Cartography;
use datasets::{month_name, DatasetStroke};
use eframe::{
    egui,
    egui::{Color32, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2},
};
use geology_diagnostics::Report as GeologyReport;
use globe3d::{GlobeCallback, GlobeMesh};
use map_layers::{BoundarySegment, LayerKind, LayerStroke};
use rainshadow::WindFrom;
use render::{atlas_geo_from_fraction, forward_globe, globe_geo_from_fraction, Theme};
use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
};
use terrain_edit::Stroke as TerrainStroke;
use terrain_generation::TerrainMode;
use world::{geo_to_uv, Project, World};

/// Return a scientifically unwarped cropped UV viewport for the native
/// equirectangular raster, independent of desktop viewport proportions.
fn atlas_crop_uv(data_aspect: f32, viewport_aspect: f32, zoom: f32, center: Pos2) -> Rect {
    let viewport_aspect = viewport_aspect.max(0.01);
    let data_aspect = data_aspect.max(0.01);
    let zoom = zoom.clamp(1.0, 8.0);
    let u_span = (viewport_aspect / data_aspect).min(1.0) / zoom;
    let v_span = (data_aspect / viewport_aspect).min(1.0) / zoom;
    let u = (center.x).clamp(u_span * 0.5, 1.0 - u_span * 0.5);
    let v = (center.y).clamp(v_span * 0.5, 1.0 - v_span * 0.5);
    Rect::from_min_max(
        egui::pos2(u - u_span * 0.5, v - v_span * 0.5),
        egui::pos2(u + u_span * 0.5, v + v_span * 0.5),
    )
}

fn source_for_theme(theme: Theme) -> Option<LayerKind> {
    match theme {
        Theme::Plates
        | Theme::Boundaries
        | Theme::PlateConvergence
        | Theme::PlateDivergence
        | Theme::PlateShear => Some(LayerKind::Plates),
        Theme::Cratons | Theme::BasementAge => Some(LayerKind::Cratons),
        Theme::Provinces => Some(LayerKind::Provinces),
        Theme::Crust => Some(LayerKind::Crust),
        Theme::Lithosphere => Some(LayerKind::Lithosphere),
        Theme::HeatFlow => Some(LayerKind::HeatFlow),
        Theme::Terrain
        | Theme::Slope
        | Theme::RainShadow
        | Theme::MonthlyTemperature
        | Theme::MonthlyPrecipitation => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Atlas,
    Globe,
}

/// Choose the authoritative elevation source before deriving map fields.
fn activate_terrain(world: &mut World, project: &Project) -> Result<TerrainMode, String> {
    project.layers.apply(world)?;
    project.datasets.activate(world, project.month)?;
    let has_edits = !project.terrain.overrides.is_empty() || project.terrain.base_fill_m.is_some();
    let mode = terrain_generation::resolve_mode(project.terrain_mode, world, has_edits);
    if mode == TerrainMode::Generated {
        let generated = terrain_generation::generate(world, &project.terrain_recipe)?;
        world.plate_motion = generated.kinematics;
        world.elevation = Some(generated.elevation);
    } else {
        // Motion maps must be available even while comparing the imported DEM.
        world.plate_motion =
            plate_kinematics::derive(world, project.terrain_recipe.planet_radius_km)?;
    }
    project.terrain.apply(world)?;
    Ok(mode)
}

struct Studio {
    world: World,
    cartography: Cartography,
    project: Project,
    project_path: PathBuf,
    imported_elevation: Option<world::Grid>,
    active_terrain_mode: TerrainMode,
    geology_report: Option<GeologyReport>,
    theme: Theme,
    view: ViewMode,
    atlas: Option<TextureHandle>,
    gpu_mesh: Arc<GlobeMesh>,
    globe_map_rgba: Arc<Vec<u8>>,
    globe_map_size: [u32; 2],
    globe_map_version: u64,
    globe_target_format: wgpu::TextureFormat,
    globe_exaggeration: f32,
    atlas_dirty: bool,
    globe_dirty: bool,
    globe_lon: f64,
    globe_lat: f64,
    zoom: f32,
    atlas_fill_canvas: bool,
    pan_u: f32,
    pan_v: f32,
    picked: Option<(f64, f64)>,
    selected_marker: Option<usize>,
    placing_marker: bool,
    terrain_brush: bool,
    terrain_raise: bool,
    terrain_radius: u32,
    terrain_strength_m: f32,
    active_stroke: TerrainStroke,
    terrain_undo: Vec<TerrainStroke>,
    terrain_redo: Vec<TerrainStroke>,
    layer_brush: bool,
    layer_radius: u32,
    layer_paint_value: f32,
    active_layer_stroke: LayerStroke,
    layer_undo: Vec<LayerStroke>,
    layer_redo: Vec<LayerStroke>,
    dataset_brush: bool,
    dataset_radius: u32,
    dataset_paint_value: f32,
    copy_from_month: u8,
    active_dataset_stroke: DatasetStroke,
    dataset_undo: Vec<DatasetStroke>,
    dataset_redo: Vec<DatasetStroke>,
    boundary_draw: bool,
    boundary_class: u8,
    boundary_start: Option<(f64, f64)>,
    changed: bool,
    status: String,
    new_project_open: bool,
    new_project_name: String,
    new_project_folder: String,
    new_project_error: Option<String>,
    discard_unsaved_changes: bool,
}

impl Studio {
    fn new(
        cc: &eframe::CreationContext<'_>,
        mut world: World,
        project: Project,
        path: PathBuf,
        mut status: String,
    ) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(22, 27, 36);
        visuals.window_fill = Color32::from_rgb(25, 31, 41);
        visuals.override_text_color = Some(Color32::from_rgb(228, 234, 240));
        visuals.selection.bg_fill = Color32::from_rgb(61, 129, 165);
        cc.egui_ctx.set_visuals(visuals);
        cc.egui_ctx.style_mut(|style| {
            style.spacing.item_spacing = Vec2::new(8.0, 8.0);
            style.spacing.button_padding = Vec2::new(12.0, 6.0);
        });
        let imported_elevation = world.elevation.clone();
        let active_terrain_mode = match activate_terrain(&mut world, &project) {
            Ok(mode) => {
                status = format!("{} · active terrain: {}", status, mode.label());
                mode
            }
            Err(error) => {
                status = format!("Could not activate terrain: {error}");
                TerrainMode::Imported
            }
        };
        world.slope_deg = world
            .elevation
            .as_ref()
            .and_then(|dem| map_graph::slope_degrees(dem, 6371.0).ok());
        world.rain_shadow = world
            .elevation
            .as_ref()
            .map(|dem| rainshadow::derive(dem, project.wind_from));
        let cartography = Cartography::new(&world);
        let gpu_mesh = Arc::new(GlobeMesh::from_world(&world));
        let globe_map_size = [world.width as u32, world.height as u32];
        let globe_target_format = cc
            .wgpu_render_state
            .as_ref()
            .map(|state| state.target_format)
            .unwrap_or(wgpu::TextureFormat::Bgra8Unorm);
        let mut app = Self {
            world,
            cartography,
            project,
            project_path: path,
            imported_elevation,
            active_terrain_mode,
            geology_report: None,
            theme: Theme::Terrain,
            view: ViewMode::Atlas,
            atlas: None,
            gpu_mesh,
            globe_map_rgba: Arc::new(Vec::new()),
            globe_map_size,
            globe_map_version: 0,
            globe_target_format,
            globe_exaggeration: 8.0,
            atlas_dirty: true,
            globe_dirty: true,
            globe_lon: 0.0,
            globe_lat: 0.0,
            zoom: 1.0,
            atlas_fill_canvas: true,
            pan_u: 0.5,
            pan_v: 0.5,
            picked: None,
            selected_marker: None,
            placing_marker: false,
            terrain_brush: false,
            terrain_raise: true,
            terrain_radius: 7,
            terrain_strength_m: 160.0,
            active_stroke: TerrainStroke::default(),
            terrain_undo: Vec::new(),
            terrain_redo: Vec::new(),
            layer_brush: false,
            layer_radius: 5,
            layer_paint_value: 1.0,
            active_layer_stroke: LayerStroke::default(),
            layer_undo: Vec::new(),
            layer_redo: Vec::new(),
            dataset_brush: false,
            dataset_radius: 5,
            dataset_paint_value: 20.0,
            copy_from_month: 1,
            active_dataset_stroke: DatasetStroke::default(),
            dataset_undo: Vec::new(),
            dataset_redo: Vec::new(),
            boundary_draw: false,
            boundary_class: 2,
            boundary_start: None,
            changed: false,
            status,
            new_project_open: false,
            new_project_name: String::new(),
            new_project_folder: String::new(),
            new_project_error: None,
            discard_unsaved_changes: false,
        };
        if !app.theme.available(&app.world) {
            app.theme = Theme::ALL
                .iter()
                .copied()
                .find(|t| t.available(&app.world))
                .unwrap_or(Theme::Terrain);
        }
        app.refresh_textures(&cc.egui_ctx);
        app
    }

    fn refresh_textures(&mut self, ctx: &egui::Context) {
        if self.atlas_dirty {
            let image = render::atlas_image(&self.world, &self.cartography, self.theme);
            let mut rgba = image
                .pixels
                .iter()
                .flat_map(|pixel| pixel.to_array())
                .collect::<Vec<_>>();
            globe3d::normalize_polar_texture(&mut rgba, image.size[0], image.size[1]);
            self.globe_map_size = [image.size[0] as u32, image.size[1] as u32];
            self.globe_map_rgba = Arc::new(rgba);
            self.globe_map_version = self.globe_map_version.wrapping_add(1);
            self.atlas = Some(ctx.load_texture("world-atlas", image, egui::TextureOptions::LINEAR));
            self.atlas_dirty = false;
        }
        if self.globe_dirty {
            ctx.request_repaint();
            self.globe_dirty = false;
        }
    }

    fn rebuild_derived_surfaces(&mut self) {
        self.world.slope_deg = self
            .world
            .elevation
            .as_ref()
            .and_then(|dem| map_graph::slope_degrees(dem, 6371.0).ok());
        self.world.rain_shadow = self
            .world
            .elevation
            .as_ref()
            .map(|dem| rainshadow::derive(dem, self.project.wind_from));
        self.cartography = Cartography::new(&self.world);
        self.gpu_mesh = Arc::new(GlobeMesh::from_world(&self.world));
        self.atlas_dirty = true;
        self.globe_dirty = true;
        self.changed = true;
    }

    fn select_terrain_mode(&mut self, new_mode: TerrainMode) {
        if !self.project.terrain.overrides.is_empty()
            || (self.project.terrain.base_fill_m.is_some() && self.imported_elevation.is_some())
        {
            self.status="Cannot change terrain basis with saved height edits. Preserve this project or clear edits before switching.".into();
            return;
        }
        let (next, plate_motion) = match new_mode {
            TerrainMode::Imported => {
                let Some(grid) = &self.imported_elevation else {
                    self.status = "No imported source DEM exists for this world".into();
                    return;
                };
                match plate_kinematics::derive(
                    &self.world,
                    self.project.terrain_recipe.planet_radius_km,
                ) {
                    Ok(motion) => (grid.clone(), motion),
                    Err(error) => {
                        self.status = format!("Plate-motion diagnostics unavailable: {error}");
                        return;
                    }
                }
            }
            TerrainMode::Generated | TerrainMode::Auto => {
                match terrain_generation::generate(&self.world, &self.project.terrain_recipe) {
                    Ok(result) => (result.elevation, result.kinematics),
                    Err(error) => {
                        self.status = format!("Geology generation unavailable: {error}");
                        return;
                    }
                }
            }
        };
        let effective = if new_mode == TerrainMode::Auto {
            TerrainMode::Generated
        } else {
            new_mode
        };
        self.world.elevation = Some(next);
        self.world.plate_motion = plate_motion;
        if effective == TerrainMode::Generated && self.imported_elevation.is_none() {
            // The pristine -2,000m blank-world initializer is superseded, not
            // layered over the new generator's DEM on the next project reopen.
            self.project.terrain.base_fill_m = None;
            self.project.terrain.grid_size = None;
        }
        self.project.terrain_mode = effective;
        self.active_terrain_mode = effective;
        self.terrain_undo.clear();
        self.terrain_redo.clear();
        self.rebuild_derived_surfaces();
        self.status = format!(
            "Now showing {} (source geological fields unchanged)",
            effective.label()
        );
    }

    fn finish_terrain_stroke(&mut self) {
        if self.active_stroke.is_empty() {
            return;
        }
        if self.project.terrain_mode == TerrainMode::Auto {
            // Lock the source selected for authored edits at the first stroke.
            self.project.terrain_mode = self.active_terrain_mode;
        }
        let stroke = std::mem::take(&mut self.active_stroke);
        self.terrain_undo.push(stroke);
        if self.terrain_undo.len() > 64 {
            self.terrain_undo.remove(0);
        }
        self.terrain_redo.clear();
        self.rebuild_derived_surfaces();
        self.status = format!(
            "Terrain edited: {} authored cells",
            self.project.terrain.overrides.len()
        );
    }

    fn undo_terrain(&mut self) {
        self.finish_terrain_stroke();
        if let Some(stroke) = self.terrain_undo.pop() {
            stroke.undo(&mut self.world, &mut self.project.terrain);
            self.terrain_redo.push(stroke);
            self.rebuild_derived_surfaces();
            self.status = "Undid terrain brush stroke".into();
        }
    }

    fn redo_terrain(&mut self) {
        self.finish_terrain_stroke();
        if let Some(stroke) = self.terrain_redo.pop() {
            stroke.redo(&mut self.world, &mut self.project.terrain);
            self.terrain_undo.push(stroke);
            self.rebuild_derived_surfaces();
            self.status = "Redid terrain brush stroke".into();
        }
    }

    fn validate_geological_inputs(&mut self) {
        let report = geology_diagnostics::examine(&self.world);
        let warnings = report.warning_count();
        let path = self.project_path.with_file_name("geology-consistency.json");
        self.status = match report.write(&path) {
            Ok(()) => format!(
                "Geology diagnostics: {warnings} warnings/errors; report at {}",
                path.display()
            ),
            Err(error) => format!("Could not save geology report: {error}"),
        };
        self.geology_report = Some(report);
    }

    fn save(&mut self) {
        match self.project.save(&self.project_path) {
            Ok(()) => {
                self.changed = false;
                self.status = format!("Saved {}", self.project_path.display());
            }
            Err(error) => self.status = format!("Could not save project: {error}"),
        }
    }

    fn begin_new_project(&mut self) {
        let mut folder = PathBuf::from("projects/untitled-world");
        let mut suffix = 2;
        while folder.exists() {
            folder = PathBuf::from(format!("projects/untitled-world-{suffix}"));
            suffix += 1;
        }
        self.new_project_name = "Untitled World".into();
        self.new_project_folder = folder.to_string_lossy().into_owned();
        self.new_project_error = None;
        self.discard_unsaved_changes = false;
        self.new_project_open = true;
    }

    fn create_new_blank_project(&mut self) {
        if self.changed && !self.discard_unsaved_changes {
            self.new_project_error =
                Some("Save the current project or confirm discarding unsaved changes.".into());
            return;
        }
        let folder = PathBuf::from(self.new_project_folder.trim());
        match Project::create_blank(folder, &self.new_project_name) {
            Ok((world, project, path)) => {
                self.world = world;
                self.project = project;
                self.project_path = path;
                self.imported_elevation = None;
                self.active_terrain_mode = TerrainMode::Imported;
                self.geology_report = None;
                self.theme = Theme::Terrain;
                self.view = ViewMode::Atlas;
                self.atlas = None;
                self.globe_map_rgba = Arc::new(Vec::new());
                self.globe_map_size = [self.world.width as u32, self.world.height as u32];
                self.globe_lon = 0.0;
                self.globe_lat = 0.0;
                self.globe_exaggeration = 8.0;
                self.zoom = 1.0;
                self.atlas_fill_canvas = true;
                self.pan_u = 0.5;
                self.pan_v = 0.5;
                self.picked = None;
                self.selected_marker = None;
                self.placing_marker = false;
                self.terrain_brush = false;
                self.terrain_raise = true;
                self.terrain_radius = 7;
                self.terrain_strength_m = 160.0;
                self.active_stroke = TerrainStroke::default();
                self.terrain_undo.clear();
                self.terrain_redo.clear();
                self.layer_brush = false;
                self.layer_radius = 5;
                self.layer_paint_value = 1.0;
                self.active_layer_stroke = LayerStroke::default();
                self.layer_undo.clear();
                self.layer_redo.clear();
                self.dataset_brush = false;
                self.dataset_radius = 5;
                self.dataset_paint_value = 20.0;
                self.copy_from_month = 1;
                self.active_dataset_stroke = DatasetStroke::default();
                self.dataset_undo.clear();
                self.dataset_redo.clear();
                self.boundary_draw = false;
                self.boundary_class = 2;
                self.boundary_start = None;
                self.rebuild_derived_surfaces();
                // The new world.json already persists its empty editing baseline.
                self.changed = false;
                self.new_project_error = None;
                self.new_project_open = false;
                self.status = format!(
                    "Created blank editable world: {} ({}×{} flat ocean)",
                    self.project_path.display(),
                    self.world.width,
                    self.world.height
                );
            }
            Err(error) => self.new_project_error = Some(error),
        }
    }

    fn new_project_dialog(&mut self, ctx: &egui::Context) {
        if !self.new_project_open {
            return;
        }
        let mut open = true;
        let mut cancel = false;
        let mut create = false;
        egui::Window::new("New blank project")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label("Start a separate world with no Aegaea geology, plates, or markers.");
                ui.small("An editable 1024×512 flat ocean is created and saved immediately.");
                ui.add_space(8.0);
                ui.label("Project name");
                ui.text_edit_singleline(&mut self.new_project_name);
                ui.label("New project folder");
                ui.text_edit_singleline(&mut self.new_project_folder);
                ui.small("Relative folders are created under the current working directory.");
                if self.changed {
                    ui.separator();
                    ui.colored_label(
                        Color32::from_rgb(229, 188, 111),
                        "The current project has unsaved changes.",
                    );
                    ui.checkbox(
                        &mut self.discard_unsaved_changes,
                        "Discard unsaved changes and create new project",
                    );
                }
                if let Some(error) = &self.new_project_error {
                    ui.colored_label(Color32::from_rgb(240, 145, 130), error);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                    let can_create = !self.new_project_name.trim().is_empty()
                        && !self.new_project_folder.trim().is_empty()
                        && (!self.changed || self.discard_unsaved_changes);
                    if ui
                        .add_enabled(can_create, egui::Button::new("Create blank project"))
                        .clicked()
                    {
                        create = true;
                    }
                });
            });
        if cancel || !open {
            self.new_project_open = false;
        } else if create {
            self.create_new_blank_project();
        }
    }

    fn refresh_layer_maps(&mut self) {
        match plate_kinematics::derive(&self.world, self.project.terrain_recipe.planet_radius_km) {
            Ok(motion) => self.world.plate_motion = motion,
            Err(error) => {
                self.world.plate_motion = None;
                self.status = format!("Euler motion not available: {error}");
            }
        }
        self.cartography = Cartography::new(&self.world);
        self.geology_report = None;
        self.atlas_dirty = true;
        self.globe_dirty = true;
        self.changed = true;
    }

    fn initialize_map_layer(&mut self, kind: LayerKind, fill: f32) {
        match self.project.layers.create(&mut self.world, kind, fill) {
            Ok(()) => {
                self.layer_paint_value = fill;
                self.refresh_layer_maps();
                self.status = format!(
                    "Initialized {} as an AUTHOR-ASSUMED layer; paint values and Save to persist",
                    kind.name()
                );
            }
            Err(error) => self.status = format!("Could not initialize layer: {error}"),
        }
    }

    fn finish_layer_stroke(&mut self) {
        if self.active_layer_stroke.is_empty() {
            return;
        }
        self.layer_undo
            .push(std::mem::take(&mut self.active_layer_stroke));
        if self.layer_undo.len() > 64 {
            self.layer_undo.remove(0);
        }
        self.layer_redo.clear();
        self.refresh_layer_maps();
    }

    fn undo_layer(&mut self) {
        self.finish_layer_stroke();
        if let Some(stroke) = self.layer_undo.pop() {
            stroke.undo(&mut self.world, &mut self.project.layers);
            self.layer_redo.push(stroke);
            self.refresh_layer_maps();
            self.status = "Undid map-layer paint stroke".into();
        }
    }

    fn redo_layer(&mut self) {
        self.finish_layer_stroke();
        if let Some(stroke) = self.layer_redo.pop() {
            stroke.redo(&mut self.world, &mut self.project.layers);
            self.layer_undo.push(stroke);
            self.refresh_layer_maps();
            self.status = "Redid map-layer paint stroke".into();
        }
    }

    fn refresh_dataset_map(&mut self) {
        self.atlas_dirty = true;
        self.globe_dirty = true;
        self.changed = true;
    }

    fn finish_dataset_stroke(&mut self) {
        if self.active_dataset_stroke.is_empty() {
            return;
        }
        self.dataset_undo.push(std::mem::take(&mut self.active_dataset_stroke));
        if self.dataset_undo.len() > 64 {
            self.dataset_undo.remove(0);
        }
        self.dataset_redo.clear();
        self.refresh_dataset_map();
    }

    fn undo_dataset(&mut self) {
        self.finish_dataset_stroke();
        if let Some(stroke) = self.dataset_undo.pop() {
            stroke.undo(&mut self.world, &mut self.project.datasets);
            self.dataset_redo.push(stroke);
            self.refresh_dataset_map();
            self.status = "Undid monthly map stroke".into();
        }
    }

    fn redo_dataset(&mut self) {
        self.finish_dataset_stroke();
        if let Some(stroke) = self.dataset_redo.pop() {
            stroke.redo(&mut self.world, &mut self.project.datasets);
            self.dataset_undo.push(stroke);
            self.refresh_dataset_map();
            self.status = "Redid monthly map stroke".into();
        }
    }

    fn select_month(&mut self, month: u8) {
        self.finish_dataset_stroke();
        match self.project.datasets.activate(&mut self.world, month) {
            Ok(()) => {
                self.project.month = month;
                self.dataset_undo.clear();
                self.dataset_redo.clear();
                self.dataset_brush = false;
                self.copy_from_month = month;
                self.refresh_dataset_map();
                self.status = format!(
                    "Showing {} · missing monthly fields remain unfilled",
                    month_name(month)
                );
            }
            Err(error) => self.status = format!("Could not select month: {error}"),
        }
    }

    fn monthly_workshop(&mut self, ui: &mut egui::Ui) {
        let Some(kind) = self.theme.climate_field() else { return };
        ui.separator();
        ui.label(egui::RichText::new("MONTHLY SCIENTIFIC DATA").strong().small());
        ui.small("Authored climate values only. No atmosphere, rainfall or energy-balance simulation.");
        let mut selected = self.project.month;
        egui::ComboBox::from_id_salt("active-month")
            .selected_text(month_name(selected))
            .show_ui(ui, |ui| {
                for month in 1..=12u8 {
                    ui.selectable_value(&mut selected, month, month_name(month));
                }
            });
        if selected != self.project.month {
            self.select_month(selected);
        }
        let exists = self.project.datasets.has_month(kind, self.project.month);
        if !exists {
            ui.small(format!(
                "No {} dataset for {}. A missing month stays missing.",
                kind.name(), month_name(self.project.month)
            ));
            ui.label(format!("Initial assumed value ({})", kind.unit().label()));
            ui.add(egui::DragValue::new(&mut self.dataset_paint_value).speed(0.5));
            let bounds = kind.bounds();
            let valid = self.dataset_paint_value.is_finite()
                && (bounds[0]..=bounds[1]).contains(&self.dataset_paint_value);
            if !valid {
                ui.colored_label(
                    Color32::from_rgb(229, 147, 122), "Value outside the field's physical limits."
                );
            }
            if ui.add_enabled(valid, egui::Button::new("Create this month's source")).clicked() {
                match self.project.datasets.create_month(
                    &mut self.world, kind, self.project.month, self.dataset_paint_value
                ) {
                    Ok(()) => {
                        self.refresh_dataset_map();
                        self.status = format!(
                            "Created {} for {} (author-assumed values)",
                            kind.name(), month_name(self.project.month)
                        );
                    }
                    Err(error) => self.status = format!("Could not create monthly field: {error}"),
                }
            }
            if let Some(field) = self.project.datasets.fields.get(kind.id()) {
                let months: Vec<u8> = field.months.keys().copied().collect();
                if !months.is_empty() {
                    if !months.contains(&self.copy_from_month) {
                        self.copy_from_month = months[0];
                    }
                    ui.separator();
                    ui.small("Or explicitly duplicate an authored month:");
                    egui::ComboBox::from_id_salt("month-to-duplicate")
                        .selected_text(month_name(self.copy_from_month))
                        .show_ui(ui, |ui| {
                            for month in &months {
                                ui.selectable_value(
                                    &mut self.copy_from_month, *month, month_name(*month)
                                );
                            }
                        });
                    if ui.button("Copy values into this month").clicked() {
                        match self.project.datasets.copy_month(
                            &mut self.world, kind, self.copy_from_month, self.project.month
                        ) {
                            Ok(()) => {
                                self.refresh_dataset_map();
                                self.status = "Month copied explicitly; no simulation performed".into();
                            }
                            Err(error) => self.status = format!("Month copy failed: {error}"),
                        }
                    }
                }
            }
            return;
        }
        let field = &self.project.datasets.fields[kind.id()];
        let snapshot = &field.months[&self.project.month];
        ui.small(format!(
            "{} · {} · {} authored cells",
            field.name, field.provenance.label(), snapshot.overrides.len()
        ));
        ui.small(format!(
            "{} source; dependencies: {}",
            field.unit.label(), field.dependencies.len()
        ));
        if ui.checkbox(&mut self.dataset_brush, "Paint monthly values").changed()
            && self.dataset_brush
        {
            self.terrain_brush = false;
            self.layer_brush = false;
            self.boundary_draw = false;
            self.boundary_start = None;
            self.placing_marker = false;
        }
        if self.dataset_brush {
            let bounds = kind.bounds();
            ui.label(format!("Paint value ({})", kind.unit().label()));
            ui.add(egui::DragValue::new(&mut self.dataset_paint_value).speed(0.5));
            if !self.dataset_paint_value.is_finite()
                || !(bounds[0]..=bounds[1]).contains(&self.dataset_paint_value)
            {
                ui.colored_label(Color32::from_rgb(229, 147, 122), "Invalid paint value.");
            }
            ui.label("Brush radius (cells)");
            ui.add(egui::Slider::new(&mut self.dataset_radius, 1..=40));
            ui.small("Drag on the atlas or globe. Missing months are never interpolated.");
        }
        ui.horizontal(|ui| {
            if ui.add_enabled(!self.dataset_undo.is_empty(), egui::Button::new("Undo climate")).clicked() {
                self.undo_dataset();
            }
            if ui.add_enabled(!self.dataset_redo.is_empty(), egui::Button::new("Redo climate")).clicked() {
                self.redo_dataset();
            }
        });
    }

    fn map_layer_workshop(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.label(egui::RichText::new("MAP LAYER WORKSHOP").small().strong());
        let Some(kind) = source_for_theme(self.theme) else {
            ui.small(match self.theme {
                Theme::Terrain => "Elevation is authored with the Terrain brush below.",
                Theme::Slope => "Derived from your elevation map; no separate source to paint.",
                Theme::RainShadow => "Derived from elevation and the chosen wind direction.",
                _ => "",
            });
            return;
        };
        if !self.project.layers.grids.contains_key(&kind) {
            ui.small(format!("Source: {} · {}", kind.name(), kind.units()));
            if kind.field(&self.world).is_some() {
                if ui
                    .button("Edit existing source non-destructively")
                    .clicked()
                {
                    self.initialize_map_layer(kind, kind.default_fill());
                }
            } else {
                ui.small("No source raster. Initialize an explicitly assumed uniform field, then paint it.");
                let mut fill = self.layer_paint_value;
                if !kind.valid_value(fill) {
                    fill = kind.default_fill();
                }
                if kind.is_categorical() {
                    fill = kind.default_fill();
                } else {
                    ui.horizontal(|ui| {
                        ui.label(format!("Initial {}", kind.units()));
                        ui.add(egui::DragValue::new(&mut fill).speed(1.0));
                    });
                }
                self.layer_paint_value = fill;
                if ui
                    .add_enabled(
                        kind.valid_value(fill),
                        egui::Button::new("Create source layer"),
                    )
                    .clicked()
                {
                    self.initialize_map_layer(kind, fill);
                }
            }
            if self.theme == Theme::PlateConvergence
                || self.theme == Theme::PlateDivergence
                || self.theme == Theme::PlateShear
            {
                ui.small("Motion also requires each plate's Euler pole and rotation rate.");
            }
            return;
        }

        let edited = self.project.layers.grids.get(&kind).unwrap();
        ui.small(format!(
            "{} · {} authored cells",
            kind.name(),
            edited.overrides.len()
        ));
        ui.small("Project-local source overlay · not simulated; original TIFFs stay unchanged.");
        if ui
            .checkbox(&mut self.layer_brush, "Paint source layer")
            .changed()
            && self.layer_brush
        {
            self.terrain_brush = false;
            self.dataset_brush = false;
            self.boundary_draw = false;
            self.boundary_start = None;
            self.placing_marker = false;
        }
        if self.layer_brush {
            match kind {
                LayerKind::Plates => {
                    egui::ComboBox::from_id_salt("plate-paint-id")
                        .selected_text(format!("Plate ID {}", self.layer_paint_value as u8))
                        .show_ui(ui, |ui| {
                            for p in &self.world.plates {
                                ui.selectable_value(
                                    &mut self.layer_paint_value,
                                    p.plate_id as f32,
                                    format!("{} · {}", p.plate_id, p.plate),
                                );
                            }
                        });
                }
                LayerKind::Cratons => {
                    egui::ComboBox::from_id_salt("craton-paint-id")
                        .selected_text(format!("Craton ID {}", self.layer_paint_value as u8))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.layer_paint_value, 0.0, "0 · No craton");
                            for c in &self.world.cratons {
                                ui.selectable_value(
                                    &mut self.layer_paint_value,
                                    c.craton_id as f32,
                                    format!("{} · {}", c.craton_id, c.name),
                                );
                            }
                        });
                }
                LayerKind::Provinces => {
                    egui::ComboBox::from_id_salt("province-paint-id")
                        .selected_text(format!("Class {}", self.layer_paint_value as u8))
                        .show_ui(ui, |ui| {
                            for p in &self.world.provinces {
                                ui.selectable_value(
                                    &mut self.layer_paint_value,
                                    p.province_id as f32,
                                    format!("{} · {}", p.province_id, p.name.replace('_', " ")),
                                );
                            }
                        });
                }
                _ => {
                    ui.label(format!("Paint value ({})", kind.units()));
                    ui.add(egui::DragValue::new(&mut self.layer_paint_value).speed(0.5));
                    if !kind.valid_value(self.layer_paint_value) {
                        ui.colored_label(
                            Color32::from_rgb(240, 145, 130),
                            "Value outside allowed range.",
                        );
                    }
                }
            }
            ui.label("Brush radius (cells)");
            ui.add(egui::Slider::new(&mut self.layer_radius, 1..=40));
            ui.small("Drag in the atlas or globe; release to commit one undoable paint stroke.");
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.layer_undo.is_empty(), egui::Button::new("Undo paint"))
                .clicked()
            {
                self.undo_layer();
            }
            if ui
                .add_enabled(!self.layer_redo.is_empty(), egui::Button::new("Redo paint"))
                .clicked()
            {
                self.redo_layer();
            }
        });
        if self.theme == Theme::Boundaries {
            ui.separator();
            ui.label(egui::RichText::new("CLASSIFIED CONTACTS").small().strong());
            ui.small(
                "Manual line classifications are authored hypotheses, not inferred plate physics.",
            );
            egui::ComboBox::from_id_salt("boundary-class")
                .selected_text(BoundarySegment::CLASSES[self.boundary_class as usize])
                .show_ui(ui, |ui| {
                    for (i, label) in BoundarySegment::CLASSES.iter().enumerate() {
                        ui.selectable_value(&mut self.boundary_class, i as u8, *label);
                    }
                });
            if ui
                .checkbox(
                    &mut self.boundary_draw,
                    "Draw classified contact (two clicks)",
                )
                .changed()
            {
                self.boundary_start = None;
                if self.boundary_draw {
                    self.layer_brush = false;
                    self.dataset_brush = false;
                    self.terrain_brush = false;
                    self.placing_marker = false;
                }
            }
            if let Some((lon, lat)) = self.boundary_start {
                ui.small(format!(
                    "First point: {lat:.1}°, {lon:.1}° · click the endpoint."
                ));
            }
            ui.small(format!(
                "{} authored classified segments",
                self.project.layers.boundary_segments.len()
            ));
            if ui
                .add_enabled(
                    !self.project.layers.boundary_segments.is_empty(),
                    egui::Button::new("Remove last contact"),
                )
                .clicked()
            {
                self.project.layers.boundary_segments.pop();
                self.world
                    .boundary_segments
                    .clone_from(&self.project.layers.boundary_segments);
                self.refresh_layer_maps();
            }
        }
        if kind == LayerKind::Plates {
            if ui.button("Add plate").clicked() {
                match self.project.layers.add_plate(&mut self.world) {
                    Ok(id) => {
                        self.layer_paint_value = id as f32;
                        self.refresh_layer_maps();
                    }
                    Err(error) => self.status = error,
                }
            }
            let mut metadata_changed = false;
            if let Some(plates) = self.project.layers.plates.as_mut() {
                for plate in plates.iter_mut() {
                    ui.collapsing(
                        format!("Plate {} · {}", plate.plate_id, plate.plate),
                        |ui| {
                            metadata_changed |= ui.text_edit_singleline(&mut plate.plate).changed();
                            ui.small("RGB (red,green,blue)");
                            metadata_changed |=
                                ui.text_edit_singleline(&mut plate.color_rgb).changed();
                            let mut has_motion = plate.signed_rate_deg_per_ma.is_some();
                            if ui
                                .checkbox(&mut has_motion, "Define Euler rotation")
                                .changed()
                            {
                                metadata_changed = true;
                                if has_motion {
                                    plate.euler_pole_lon_deg = Some(0.0);
                                    plate.euler_pole_lat_deg = Some(0.0);
                                    plate.signed_rate_deg_per_ma = Some(0.0);
                                } else {
                                    plate.euler_pole_lon_deg = None;
                                    plate.euler_pole_lat_deg = None;
                                    plate.signed_rate_deg_per_ma = None;
                                }
                            }
                            if let (Some(lon), Some(lat), Some(rate)) = (
                                plate.euler_pole_lon_deg.as_mut(),
                                plate.euler_pole_lat_deg.as_mut(),
                                plate.signed_rate_deg_per_ma.as_mut(),
                            ) {
                                ui.label("Pole longitude (degrees)");
                                metadata_changed |=
                                    ui.add(egui::Slider::new(lon, -180.0..=180.0)).changed();
                                ui.label("Pole latitude (degrees)");
                                metadata_changed |=
                                    ui.add(egui::Slider::new(lat, -90.0..=90.0)).changed();
                                ui.label("Signed rate (degrees / Ma)");
                                metadata_changed |=
                                    ui.add(egui::Slider::new(rate, -20.0..=20.0)).changed();
                            }
                        },
                    );
                }
                if metadata_changed {
                    self.world.plates.clone_from(plates);
                }
            }
            if metadata_changed {
                self.refresh_layer_maps();
            }
        }
        if kind == LayerKind::Cratons {
            if ui.button("Add craton").clicked() {
                match self.project.layers.add_craton(&mut self.world) {
                    Ok(id) => {
                        self.layer_paint_value = id as f32;
                        self.refresh_layer_maps();
                    }
                    Err(error) => self.status = error,
                }
            }
            let mut metadata_changed = false;
            if let Some(cratons) = self.project.layers.cratons.as_mut() {
                for craton in cratons.iter_mut() {
                    ui.collapsing(
                        format!("Craton {} · {}", craton.craton_id, craton.name),
                        |ui| {
                            metadata_changed |= ui.text_edit_singleline(&mut craton.name).changed();
                            ui.label("Parent plate");
                            egui::ComboBox::from_id_salt(("craton-parent", craton.craton_id))
                                .selected_text(&craton.plate)
                                .show_ui(ui, |ui| {
                                    for plate in &self.world.plates {
                                        metadata_changed |= ui
                                            .selectable_value(
                                                &mut craton.plate,
                                                plate.plate.clone(),
                                                &plate.plate,
                                            )
                                            .changed();
                                    }
                                });
                            ui.label("Basement age (Ga) · author assumption");
                            metadata_changed |= ui
                                .add(egui::Slider::new(&mut craton.basement_age_ga, 0.0..=4.6))
                                .changed();
                        },
                    );
                }
                if metadata_changed {
                    self.world.cratons.clone_from(cratons);
                }
            }
            if metadata_changed {
                self.refresh_layer_maps();
            }
        }
    }

    fn topbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("AEGAEA");
                ui.label(
                    egui::RichText::new("STUDIO")
                        .small()
                        .color(Color32::from_rgb(129, 167, 187)),
                );
                ui.separator();
                ui.label(egui::RichText::new(&self.project.name).strong());
                if ui.button("New project…").clicked() {
                    self.begin_new_project();
                }
                if self.changed {
                    ui.label(
                        egui::RichText::new("● Unsaved").color(Color32::from_rgb(229, 188, 111)),
                    );
                }
                ui.separator();
                if ui
                    .selectable_label(self.view == ViewMode::Atlas, "2D Atlas")
                    .clicked()
                {
                    self.view = ViewMode::Atlas;
                }
                if ui
                    .selectable_label(self.view == ViewMode::Globe, "3D Globe")
                    .clicked()
                {
                    self.view = ViewMode::Globe;
                    self.globe_dirty = true;
                }
                ui.separator();
                if ui
                    .button(if self.placing_marker {
                        "Click map to place point…"
                    } else {
                        "Add point"
                    })
                    .clicked()
                {
                    self.placing_marker = !self.placing_marker;
                    if self.placing_marker {
                        self.terrain_brush = false;
                        self.layer_brush = false;
                        self.dataset_brush = false;
                        self.boundary_draw = false;
                        self.boundary_start = None;
                    }
                }
                if ui.button("Save").clicked() {
                    self.save();
                }
                if ui
                    .add_enabled(
                        !self.terrain_undo.is_empty(),
                        egui::Button::new("Undo terrain"),
                    )
                    .clicked()
                {
                    self.undo_terrain();
                }
                if ui
                    .add_enabled(
                        !self.terrain_redo.is_empty(),
                        egui::Button::new("Redo terrain"),
                    )
                    .clicked()
                {
                    self.redo_terrain();
                }
                if ui.button("Export atlas sheet SVG").clicked() {
                    let target = self
                        .project_path
                        .with_file_name(format!("{}-atlas.svg", self.theme.slug()));
                    match publication::export_sheet(
                        &self.world,
                        &self.cartography,
                        self.theme,
                        &self.project.name,
                        &target,
                    ) {
                        Ok(()) => {
                            self.status = format!("Exported atlas sheet: {}", target.display())
                        }
                        Err(error) => self.status = format!("Atlas export failed: {error}"),
                    }
                }
                if ui.button("Export map PNG").clicked() {
                    let target = self
                        .project_path
                        .with_file_name(format!("{}.png", self.theme.slug()));
                    match render::export_atlas_png(
                        &self.world,
                        &self.cartography,
                        self.theme,
                        &target,
                    ) {
                        Ok(()) => {
                            self.status = format!(
                                "Exported native-resolution map and legend: {}",
                                target.display()
                            )
                        }
                        Err(error) => self.status = format!("Map export failed: {error}"),
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("PHYSICAL WORLD ATLAS").small().weak());
                });
            });
        });
    }

    fn layers_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("layers").default_width(250.0).min_width(205.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("layer-workspace-scroll").show(ui, |ui| {
            ui.heading("Map library");
            ui.label(egui::RichText::new("THEMATIC VIEW").small().weak());
            ui.add_space(8.0);
            egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
            for theme in Theme::ALL {
                let enabled=theme.available(&self.world);
                let text=if enabled { theme.name().to_string() } else { format!("{}  · needs source",theme.name()) };
                if ui.add(egui::Button::selectable(self.theme==theme,text)).clicked() {
                    self.finish_layer_stroke();
                    self.finish_dataset_stroke();
                    self.theme=theme;
                    self.layer_brush=false;
                    self.dataset_brush=false;
                    if let Some(field) = theme.climate_field() {
                        self.dataset_paint_value = field.default_fill();
                    }
                    self.boundary_draw=false;
                    self.boundary_start=None;
                    if let Some(kind)=source_for_theme(theme) {
                        self.layer_paint_value = match kind {
                            LayerKind::Plates => self.world.plates.first().map(|p| p.plate_id as f32).unwrap_or(1.0),
                            LayerKind::Cratons => 0.0,
                            LayerKind::Provinces => 0.0,
                            _ => kind.default_fill(),
                        };
                    }
                    self.atlas_dirty=true;
                    self.globe_dirty=true;
                }
            }
            });
            if self.theme.climate_field().is_some() {
                self.monthly_workshop(ui);
            } else {
                self.map_layer_workshop(ui);
            }
            if !self.theme.available(&self.world) {
                ui.colored_label(Color32::from_rgb(229, 188, 111),
                    "This map has no source data yet. Set up its input layer above.");
            }
            ui.separator();
            ui.collapsing(format!("Map legend · {}",self.theme.units()), |ui| {
                egui::ScrollArea::vertical().max_height(200.0).show(ui,|ui| {
                    for item in self.cartography.legend(&self.world,self.theme) {
                        ui.horizontal(|ui| {
                            ui.colored_label(Color32::from_rgb(item.rgb[0],item.rgb[1],item.rgb[2]),"■");
                            ui.small(item.label);
                        });
                    }
                });
            });
            if self.theme == Theme::RainShadow {
                ui.separator();
                ui.label(egui::RichText::new("OROGRAPHIC EXPOSURE").small().strong());
                ui.small("Wind arrives from:");
                let mut changed = false;
                egui::ComboBox::from_id_salt("rainshadow-wind")
                    .selected_text(self.project.wind_from.label())
                    .show_ui(ui, |ui| {
                        for wind in WindFrom::ALL {
                            changed |= ui.selectable_value(&mut self.project.wind_from, wind, wind.label()).changed();
                        }
                    });
                if changed {
                    self.world.rain_shadow = self.world.elevation.as_ref()
                        .map(|dem| rainshadow::derive(dem, self.project.wind_from));
                    self.atlas_dirty = true;
                    self.globe_dirty = true;
                    self.changed = true;
                    self.status = format!("Rain-shadow exposure recalculated; wind from {}",
                        self.project.wind_from.label());
                }
                ui.small("Blue/teal: windward ascent");
                ui.small("Gray: weak terrain effect");
                ui.small("Orange/red: leeward shadow");
                ui.small("Signed index (-1 to +1), not rainfall or a climate simulation.");
                if ui.button("Export index TIFF + JSON").clicked() {
                    if let Some(ref grid) = self.world.rain_shadow {
                        let target = self.project_path.with_file_name("rain-shadow-index.tif");
                        match rainshadow::export_tiff(grid, self.project.wind_from, &target) {
                            Ok(()) => self.status = format!("Exported rain-shadow index: {}", target.display()),
                            Err(error) => self.status = format!("Rain-shadow export failed: {error}"),
                        }
                    }
                }
            }
            ui.separator();
            ui.collapsing("TERRAIN SOURCE / GENERATOR",|ui| {
                ui.label(format!("Active: {}",self.active_terrain_mode.label()));
                let can_change_basis=self.project.terrain.overrides.is_empty()
                    && (self.project.terrain.base_fill_m.is_none() || self.imported_elevation.is_none());
                ui.small("Generated height uses crustal provinces, plates, crust and cratons; not the legacy elevation TIFF.");
                if ui.add_enabled(can_change_basis && terrain_generation::available(&self.world),
                    egui::Button::new("Generate from geology")).clicked() {
                    self.select_terrain_mode(TerrainMode::Generated);
                }
                if ui.add_enabled(can_change_basis && self.imported_elevation.is_some(),
                    egui::Button::new("View legacy DEM")).clicked() {
                    self.select_terrain_mode(TerrainMode::Imported);
                }
                ui.label("Generation seed");
                if ui.add_enabled(can_change_basis,egui::DragValue::new(&mut self.project.terrain_recipe.seed).speed(1)).changed() {
                    self.changed=true;
                }
                ui.label("Relief strength");
                if ui.add_enabled(can_change_basis,egui::Slider::new(
                    &mut self.project.terrain_recipe.relief_strength,0.0..=2.5
                )).changed() {
                    self.changed=true;
                }
                ui.small("After changing parameters, press Generate from geology. Project recipe is saved.");
                if !can_change_basis {
                    ui.small("Existing painted terrain edits lock the source and recipe.");
                }
            });
            ui.separator();
            ui.label(egui::RichText::new("TERRAIN EDITOR").small().strong());
            if self.world.elevation.is_none() {
                if ui.button("Create flat ocean (-2,000 m)").clicked() {
                    match self.project.terrain.create_flat_world(&mut self.world,-2000.0) {
                        Ok(())=> {
                            self.project.terrain_mode=TerrainMode::Imported;
                            self.active_terrain_mode=TerrainMode::Imported;
                            self.rebuild_derived_surfaces();
                            self.status="Created a new editable flat-ocean elevation layer".into();
                        },
                        Err(error)=>self.status=format!("Could not create terrain: {error}"),
                    }
                }
            } else {
                if ui.checkbox(&mut self.terrain_brush,"Terrain brush").clicked() && self.terrain_brush {
                    self.layer_brush=false;
                    self.dataset_brush=false;
                    self.placing_marker=false;
                }
                if self.terrain_brush {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.terrain_raise,true,"Raise");
                        ui.selectable_value(&mut self.terrain_raise,false,"Lower");
                    });
                    ui.label("Brush radius (grid cells)");
                    ui.add(egui::Slider::new(&mut self.terrain_radius,1..=30));
                    ui.label("Strength (metres per dab)");
                    ui.add(egui::Slider::new(&mut self.terrain_strength_m,25.0..=800.0));
                    ui.small("Drag to sculpt. One drag = one undoable stroke. Original DEM files are never changed.");
                }
                ui.small(format!("{} project-local edited cells",self.project.terrain.overrides.len()));
                if ui.button("Export authored DEM TIFF").clicked() {
                    let target=self.project_path.with_file_name("authored-elevation.tif");
                    match terrain_edit::export_tiff(&self.world,&self.project.terrain,&target) {
                        Ok(())=>self.status=format!("Exported numeric edited DEM: {}",target.display()),
                        Err(error)=>self.status=format!("Terrain export failed: {error}"),
                    }
                }
            }
            ui.separator();
            ui.label(egui::RichText::new("VIEW CONTROLS").small().weak());
            if self.view==ViewMode::Atlas {
                ui.checkbox(&mut self.atlas_fill_canvas,"Fill workspace");
                ui.label("Zoom");
                ui.add(egui::Slider::new(&mut self.zoom,1.0..=8.0));
                if ui.button("Fit entire planet").clicked() {
                    self.atlas_fill_canvas=false;
                    self.zoom=1.0;
                    self.pan_u=0.5;
                    self.pan_v=0.5;
                }
                ui.small("Fill crops the map instead of distorting it. Drag to pan; scroll to zoom.");
            } else {
                ui.label("Globe latitude");
                if ui.add(egui::Slider::new(&mut self.globe_lat,-85.0..=85.0).suffix("°")).changed() {
                    self.globe_dirty=true;
                }
                ui.label("Globe longitude");
                if ui.add(egui::Slider::new(&mut self.globe_lon,-180.0..=180.0).suffix("°")).changed() {
                    self.globe_dirty=true;
                }
                if ui.button("Reset orientation").clicked() {
                    self.globe_lat=0.0; self.globe_lon=0.0; self.globe_dirty=true;
                }
                ui.small("Drag the globe to rotate.");
                ui.label("Height exaggeration");
                ui.add(egui::Slider::new(&mut self.globe_exaggeration,1.0..=60.0).suffix("×"));
                ui.small("Geometric elevation from the DEM; exaggerated to reveal relief on a planetary scale.");
            }
            ui.separator();
            ui.collapsing("Geology consistency",|ui| {
                ui.small("Checks input plate, craton and province IDs. It never changes geology.");
                if ui.button("Validate source layers").clicked() {
                    self.validate_geological_inputs();
                }
                if let Some(ref report)=self.geology_report {
                    ui.small(format!("{} warnings/errors · {} native cells",
                        report.warning_count(),report.counts.total_cells));
                    ui.small(format!("Craton/plate conflicts: {} cells",
                        report.counts.conflicting_craton_plate_cells));
                    for issue in &report.issues {
                        ui.separator();
                        ui.label(egui::RichText::new(&issue.code).strong().small());
                        ui.small(&issue.description);
                        if issue.affected_cells>0 {
                            ui.monospace(format!("{} affected cells",issue.affected_cells));
                        }
                    }
                }
            });
            ui.separator();
            ui.label(egui::RichText::new("DATASET").small().weak());
            ui.small(format!("{} × {} native cells", self.world.width,self.world.height));
            ui.small(format!("{} plates · {} cratons",self.world.plates.len(),self.world.cratons.len()));
            if self.world.elevation.is_none() && self.world.has_any_source() {
                ui.colored_label(Color32::from_rgb(230,170,112),
                    "Elevation absent. Rain-shadow map requires a valid DEM.");
            }
            if self.world.plate_ids.is_none()
                && self.world.craton_ids.is_none()
                && self.world.province_ids.is_none()
            {
                ui.small("Independent world. Sculpt terrain now; geology can be added later.");
            }
            ui.separator();
            ui.label(egui::RichText::new("DATA ETHICS").small().weak());
            ui.small("Geological values are fictional scenario data, not measured Earth observations.");
            });
        });
    }

    fn inspector(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("inspector")
            .default_width(282.0)
            .min_width(235.0)
            .show(ctx, |ui| {
                ui.heading("Inspector");
                ui.label(egui::RichText::new("WORLD DATA").small().weak());
                ui.add_space(12.0);
                if let Some((lon, lat)) = self.picked {
                    egui::Grid::new("coords")
                        .num_columns(2)
                        .spacing([12.0, 9.0])
                        .show(ui, |ui| {
                            ui.label("Longitude");
                            ui.monospace(format!("{lon:.3}°"));
                            ui.end_row();
                            ui.label("Latitude");
                            ui.monospace(format!("{lat:.3}°"));
                            ui.end_row();
                            if self.theme == Theme::Slope {
                                if let Some(ref field) = self.world.slope_deg {
                                    ui.label("Surface slope");
                                    ui.monospace(format!("{:.2}°", field.geo(lon, lat)));
                                    ui.end_row();
                                }
                            }
                            if let Some(ref motion) = self.world.plate_motion {
                                let selected = match self.theme {
                                    Theme::PlateConvergence => {
                                        Some(("Convergence", &motion.convergence_cm_yr))
                                    }
                                    Theme::PlateDivergence => {
                                        Some(("Spreading", &motion.divergence_cm_yr))
                                    }
                                    Theme::PlateShear => Some(("Shear", &motion.shear_cm_yr)),
                                    _ => None,
                                };
                                if let Some((label, field)) = selected {
                                    ui.label(label);
                                    ui.monospace(format!("{:.2} cm/year", field.geo(lon, lat)));
                                    ui.end_row();
                                }
                            }
                            if let Some(h) = self.world.elevation_at(lon, lat) {
                                ui.label("Elevation");
                                ui.monospace(format!("{h:.0} m"));
                                ui.end_row();
                            }
                            if self.theme == Theme::RainShadow {
                                if let Some(index) = self.world.rain_shadow_at(lon, lat) {
                                    ui.label("Orographic exposure");
                                    ui.monospace(if !index.is_finite() {
                                        "ocean / NODATA".to_owned()
                                    } else {
                                        format!("{index:+.3}")
                                    });
                                    ui.end_row();
                                    ui.label("Wind from");
                                    ui.label(self.project.wind_from.label());
                                    ui.end_row();
                                }
                            }
                            if let Some(plate) = self.world.plate_at(lon, lat) {
                                ui.label("Plate");
                                ui.label(&plate.plate);
                                ui.end_row();
                            }
                            if let (Some(grid), true) =
                                (&self.world.province_ids, !self.world.provinces.is_empty())
                            {
                                let id = grid.geo(lon, lat) as u8;
                                if let Some(province) =
                                    self.world.provinces.iter().find(|p| p.province_id == id)
                                {
                                    ui.label("Crustal province");
                                    ui.label(province.name.replace('_', " "));
                                    ui.end_row();
                                }
                            }
                            if let Some(craton) = self.world.craton_at(lon, lat) {
                                ui.label("Craton");
                                ui.label(&craton.name);
                                ui.end_row();
                                ui.label("Basement age");
                                ui.label(format!("{:.2} Ga", craton.basement_age_ga));
                                ui.end_row();
                                ui.label("Craton plate");
                                ui.label(&craton.plate);
                                ui.end_row();
                            }
                            if let Some(g) = &self.world.crust_km {
                                ui.label("Crust");
                                ui.label(format!("{:.1} km", g.geo(lon, lat)));
                                ui.end_row();
                            }
                            if let Some(g) = &self.world.lithosphere_km {
                                ui.label("Lithosphere");
                                ui.label(format!("{:.1} km", g.geo(lon, lat)));
                                ui.end_row();
                            }
                            if let Some(g) = &self.world.heatflow {
                                ui.label("Heat flow");
                                ui.label(format!("{:.1} mW/m²", g.geo(lon, lat)));
                                ui.end_row();
                            }
                        });
                } else {
                    ui.label("Select a location on the map to inspect the scientific fields.");
                }
                ui.separator();
                ui.label(egui::RichText::new("ANNOTATIONS").small().weak());
                if self.project.markers.is_empty() {
                    ui.small("No points yet. Choose 'Add point' and click the map.");
                }
                for (index, marker) in self.project.markers.iter().enumerate() {
                    if ui
                        .selectable_label(self.selected_marker == Some(index), &marker.name)
                        .clicked()
                    {
                        self.selected_marker = Some(index);
                        self.picked = Some((marker.longitude_deg, marker.latitude_deg));
                    }
                }
                ui.add_space(7.0);
                if let Some(idx) = self.selected_marker {
                    if let Some(m) = self.project.markers.get_mut(idx) {
                        ui.label("Name");
                        if ui.text_edit_singleline(&mut m.name).changed() {
                            self.changed = true;
                        }
                        ui.label("Notes");
                        if ui.text_edit_multiline(&mut m.description).changed() {
                            self.changed = true;
                        }
                        ui.small(format!("{}°, {}°", m.latitude_deg, m.longitude_deg));
                    }
                    if ui.button("Delete selected point").clicked() {
                        self.project.markers.remove(idx);
                        self.selected_marker = None;
                        self.changed = true;
                    }
                }
            });
    }

    fn current_uv(&self, viewport_aspect: f32) -> Rect {
        let data_aspect = self.world.width as f32 / self.world.height as f32;
        atlas_crop_uv(
            data_aspect,
            viewport_aspect,
            self.zoom,
            egui::pos2(self.pan_u, self.pan_v),
        )
    }

    fn canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let available = ui.available_size();
            let usable_height = (available.y - 52.0).max(5.0);
            let ratio = self.world.width as f32 / self.world.height as f32;
            let (width, height) = match self.view {
                ViewMode::Globe => {
                    let size = available.x.min(usable_height).max(5.0);
                    (size, size)
                }
                ViewMode::Atlas if self.atlas_fill_canvas => (available.x.max(5.0), usable_height),
                ViewMode::Atlas => {
                    let width = available.x.min(usable_height * ratio).max(5.0);
                    (width, width / ratio)
                }
            };
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(format!("{}  ·  {}", self.project.name, self.theme.name()))
                        .strong(),
                );
                if !self.theme.available(&self.world) {
                    ui.colored_label(
                        Color32::from_rgb(229, 188, 111),
                        "Map data not initialized; use Map Layer Workshop on the left.",
                    );
                }
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(width, height), Sense::click_and_drag());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 4.0, Color32::from_rgb(17, 23, 33));
                let uv = self.current_uv(rect.width() / rect.height());
                match self.view {
                    ViewMode::Atlas => {
                        if let Some(ref image) = self.atlas {
                            painter.image(image.id(), rect, uv, Color32::WHITE);
                        }
                    }
                    ViewMode::Globe => {
                        painter.add(egui_wgpu::Callback::new_paint_callback(
                            rect,
                            GlobeCallback {
                                mesh: Arc::clone(&self.gpu_mesh),
                                map_rgba: Arc::clone(&self.globe_map_rgba),
                                map_size: self.globe_map_size,
                                map_version: self.globe_map_version,
                                target_format: self.globe_target_format,
                                center_lon_deg: self.globe_lon,
                                center_lat_deg: self.globe_lat,
                                vertical_exaggeration: self.globe_exaggeration,
                            },
                        ));
                    }
                }
                for (i, marker) in self.project.markers.iter().enumerate() {
                    let pos = match self.view {
                        ViewMode::Atlas => {
                            let (u, v) = geo_to_uv(marker.longitude_deg, marker.latitude_deg);
                            if u < uv.min.x as f64
                                || u > uv.max.x as f64
                                || v < uv.min.y as f64
                                || v > uv.max.y as f64
                            {
                                None
                            } else {
                                Some(egui::pos2(
                                    rect.min.x
                                        + ((u - uv.min.x as f64) / (uv.width() as f64)) as f32
                                            * rect.width(),
                                    rect.min.y
                                        + ((v - uv.min.y as f64) / (uv.height() as f64)) as f32
                                            * rect.height(),
                                ))
                            }
                        }
                        ViewMode::Globe => forward_globe(
                            marker.longitude_deg,
                            marker.latitude_deg,
                            self.globe_lon,
                            self.globe_lat,
                        )
                        .map(|(x, y)| {
                            egui::pos2(
                                rect.center().x + x * rect.width() * 0.5,
                                rect.center().y - y * rect.height() * 0.5,
                            )
                        }),
                    };
                    if let Some(pos) = pos {
                        painter.circle_filled(pos, 5.0, Color32::from_rgb(253, 195, 91));
                        painter.circle_stroke(
                            pos,
                            7.5,
                            Stroke::new(
                                if self.selected_marker == Some(i) {
                                    2.0_f32
                                } else {
                                    1.0_f32
                                },
                                Color32::WHITE,
                            ),
                        );
                    }
                }
                if !self.terrain_brush
                    && !self.layer_brush
                    && self.view == ViewMode::Globe
                    && response.dragged()
                {
                    let delta = ui.input(|input| input.pointer.delta());
                    if delta != Vec2::ZERO {
                        self.globe_lon = (self.globe_lon - (delta.x as f64) * 0.45 + 180.0)
                            .rem_euclid(360.0)
                            - 180.0;
                        self.globe_lat =
                            (self.globe_lat + (delta.y as f64) * 0.45).clamp(-85.0, 85.0);
                        self.globe_dirty = true;
                    }
                } else if !self.terrain_brush
                    && !self.layer_brush
                    && self.view == ViewMode::Atlas
                    && response.dragged()
                {
                    let delta = ui.input(|input| input.pointer.delta());
                    if delta != Vec2::ZERO {
                        self.pan_u -= delta.x / rect.width() * uv.width();
                        self.pan_v -= delta.y / rect.height() * uv.height();
                    }
                }
                if !self.terrain_brush
                    && !self.layer_brush
                    && self.view == ViewMode::Atlas
                    && response.hovered()
                {
                    let wheel = ui.input(|input| input.raw_scroll_delta.y);
                    if wheel.abs() > 0.0 {
                        self.zoom = (self.zoom * (wheel * 0.0015).exp()).clamp(1.0, 8.0);
                    }
                }
                if self.terrain_brush && (response.clicked() || response.dragged()) {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let fraction_x = ((pos.x - rect.left()) / rect.width()) as f64;
                        let fraction_y = ((pos.y - rect.top()) / rect.height()) as f64;
                        let geo = match self.view {
                            ViewMode::Atlas => Some(atlas_geo_from_fraction(
                                uv.min.x as f64 + fraction_x * uv.width() as f64,
                                uv.min.y as f64 + fraction_y * uv.height() as f64,
                            )),
                            ViewMode::Globe => globe_geo_from_fraction(
                                fraction_x,
                                fraction_y,
                                self.globe_lon,
                                self.globe_lat,
                            ),
                        };
                        if let Some((lon, lat)) = geo {
                            let signed_strength = if self.terrain_raise {
                                self.terrain_strength_m
                            } else {
                                -self.terrain_strength_m
                            };
                            match terrain_edit::brush_dab(
                                &mut self.world,
                                &mut self.project.terrain,
                                lon,
                                lat,
                                self.terrain_radius,
                                signed_strength,
                            ) {
                                Ok(dab) => {
                                    if !dab.is_empty() {
                                        self.active_stroke.merge(dab);
                                        self.changed = true;
                                        self.picked = Some((lon, lat));
                                    }
                                }
                                Err(error) => {
                                    self.status = format!("Terrain brush failed: {error}")
                                }
                            }
                        }
                    }
                }
                if self.terrain_brush && (response.clicked() || response.drag_stopped()) {
                    self.finish_terrain_stroke();
                }
                if self.layer_brush && (response.clicked() || response.dragged()) {
                    if let (Some(kind), Some(pos)) = (
                        source_for_theme(self.theme),
                        response.interact_pointer_pos(),
                    ) {
                        let fraction_x = ((pos.x - rect.left()) / rect.width()) as f64;
                        let fraction_y = ((pos.y - rect.top()) / rect.height()) as f64;
                        let geo = match self.view {
                            ViewMode::Atlas => Some(atlas_geo_from_fraction(
                                uv.min.x as f64 + fraction_x * uv.width() as f64,
                                uv.min.y as f64 + fraction_y * uv.height() as f64,
                            )),
                            ViewMode::Globe => globe_geo_from_fraction(
                                fraction_x,
                                fraction_y,
                                self.globe_lon,
                                self.globe_lat,
                            ),
                        };
                        if let Some((lon, lat)) = geo {
                            match self.project.layers.paint(
                                &mut self.world,
                                kind,
                                lon,
                                lat,
                                self.layer_radius,
                                self.layer_paint_value,
                            ) {
                                Ok(dab) => {
                                    if !dab.is_empty() {
                                        self.active_layer_stroke.merge(dab);
                                        self.changed = true;
                                        self.picked = Some((lon, lat));
                                    }
                                }
                                Err(error) => self.status = format!("Map paint failed: {error}"),
                            }
                        }
                    }
                }
                if self.layer_brush && (response.clicked() || response.drag_stopped()) {
                    self.finish_layer_stroke();
                }
                if self.layer_brush && response.hovered() {
                    if let Some(pos) = response.hover_pos() {
                        painter.circle_stroke(
                            pos,
                            10.0,
                            Stroke::new(1.5_f32, Color32::from_rgb(126, 206, 180)),
                        );
                    }
                }

                if self.terrain_brush && response.hovered() {
                    if let Some(pos) = response.hover_pos() {
                        painter.circle_stroke(
                            pos,
                            10.0,
                            Stroke::new(1.5_f32, Color32::from_rgb(250, 215, 117)),
                        );
                    }
                }
                if !self.terrain_brush && !self.layer_brush && response.clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let fraction_x = ((pos.x - rect.left()) / rect.width()) as f64;
                        let fraction_y = ((pos.y - rect.top()) / rect.height()) as f64;
                        let geo = match self.view {
                            ViewMode::Atlas => Some(atlas_geo_from_fraction(
                                uv.min.x as f64 + fraction_x * (uv.width() as f64),
                                uv.min.y as f64 + fraction_y * (uv.height() as f64),
                            )),
                            ViewMode::Globe => globe_geo_from_fraction(
                                fraction_x,
                                fraction_y,
                                self.globe_lon,
                                self.globe_lat,
                            ),
                        };
                        if let Some((lon, lat)) = geo {
                            self.picked = Some((lon, lat));
                            if self.boundary_draw {
                                if let Some((start_lon, start_lat)) = self.boundary_start.take() {
                                    let segment = BoundarySegment {
                                        start: [start_lon, start_lat],
                                        end: [lon, lat],
                                        class: self.boundary_class,
                                    };
                                    match segment.validate() {
                                        Ok(()) => {
                                            self.project.layers.boundary_segments.push(segment.clone());
                                            self.world.boundary_segments.push(segment);
                                            self.refresh_layer_maps();
                                            self.status = "Saved manually classified contact segment".into();
                                        }
                                        Err(error) => self.status = format!("Invalid contact: {error}"),
                                    }
                                } else {
                                    self.boundary_start = Some((lon, lat));
                                    self.status = "First contact endpoint selected; click the second endpoint".into();
                                }
                            } else if self.placing_marker {
                                let idx = self.project.add_marker(lon, lat);
                                self.selected_marker = Some(idx);
                                self.changed = true;
                                self.placing_marker = false;
                            } else {
                                let nearest = self
                                    .project
                                    .markers
                                    .iter()
                                    .enumerate()
                                    .find(|(_, m)| {
                                        let dlon = (m.longitude_deg - lon + 180.0)
                                            .rem_euclid(360.0)
                                            - 180.0;
                                        dlon.abs() < 3.0 && (m.latitude_deg - lat).abs() < 3.0
                                    })
                                    .map(|(i, _)| i);
                                self.selected_marker = nearest;
                            }
                        }
                    }
                }
                ui.label(
                    egui::RichText::new(if self.view == ViewMode::Atlas {
                        "Mirrored and inverted original atlas · drag to pan"
                    } else {
                        "Orthographic globe · drag to rotate"
                    })
                    .small()
                    .weak(),
                );
            });
        });
    }
}

impl eframe::App for Studio {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let undo = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z));
        let redo = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Y));
        if undo {
            if self.dataset_brush {
                self.undo_dataset();
            } else if self.layer_brush {
                self.undo_layer();
            } else {
                self.undo_terrain();
            }
        }
        if redo {
            if self.dataset_brush {
                self.redo_dataset();
            } else if self.layer_brush {
                self.redo_layer();
            } else {
                self.redo_terrain();
            }
        }
        self.topbar(ctx);
        self.layers_panel(ctx);
        self.inspector(ctx);
        self.refresh_textures(ctx);
        self.canvas(ctx);
        self.new_project_dialog(ctx);
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&self.status).small().weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((lon, lat)) = self.picked {
                        ui.monospace(format!("{lat:.2}°  {lon:.2}°"));
                    } else {
                        ui.label("No location selected");
                    }
                });
            });
        });
    }
}

fn repo_dataset_dir() -> Option<PathBuf> {
    let mut location = env::current_dir().ok()?;
    for _ in 0..6 {
        let path = location.join("data/aegaea/tectonics/v1");
        if path.join("plate_id.tif").exists() {
            return Some(path);
        }
        if !location.pop() {
            break;
        }
    }
    None
}

fn boot() -> (World, Project, PathBuf, String) {
    let args = env::args().collect::<Vec<_>>();
    let new = args
        .iter()
        .position(|a| a == "--new")
        .and_then(|i| args.get(i + 1));
    let existing = args
        .iter()
        .position(|a| a == "--project")
        .and_then(|i| args.get(i + 1));
    if let Some(folder) = new {
        let dir = PathBuf::from(folder);
        let world = World::empty(dir.clone());
        let path = dir.join("world.json");
        let project =
            Project::load(&path, "New World").unwrap_or_else(|_| Project::new("New World"));
        return (
            world,
            project,
            path,
            format!("Blank world project: {}", dir.display()),
        );
    }
    let folder = existing.map(PathBuf::from).or_else(repo_dataset_dir);
    if let Some(root) = folder {
        let (path, name) = if existing.is_some() {
            (root.join("world.json"), "Custom World")
        } else {
            (PathBuf::from("projects/aegaea-workspace.json"), "Aegaea")
        };
        let project = Project::load(&path, name).unwrap_or_else(|_| Project::new(name));
        match World::load(root.clone()) {
            Ok(world) => {
                let note = if world.elevation.is_some() {
                    "Geology inputs loaded; generated terrain is the default for untouched projects"
                        .to_owned()
                } else if world.has_any_source() {
                    "Partial geology loaded; elevation asset needs restoration".to_owned()
                } else {
                    "Blank dataset; import geological layers to populate it".to_owned()
                };
                (world, project, path, note)
            }
            Err(e) => (
                World::empty(root),
                project,
                path,
                format!("Source data load failed: {e}"),
            ),
        }
    } else {
        let root = PathBuf::from("projects/untitled");
        let path = root.join("world.json");
        (
            World::empty(root),
            Project::new("Untitled World"),
            path,
            "No reference data found. Run from repository root or use --project PATH".to_owned(),
        )
    }
}

fn main() -> eframe::Result {
    let (mut world, project, path, status) = boot();
    let args = env::args().collect::<Vec<_>>();
    if let Some(index) = args.iter().position(|a| a == "--diagnose-world") {
        let target = args
            .get(index + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("maps/diagnostics/geology-consistency.json"));
        if let Err(error) = project.layers.apply(&mut world) {
            eprintln!("Could not load authored project layers: {error}");
            std::process::exit(2);
        }
        let report = geology_diagnostics::examine(&world);
        if let Err(error) = report.write(&target) {
            eprintln!("Could not export geology diagnostics: {error}");
            std::process::exit(2);
        }
        println!(
            "Geology checked {} cells: {} warnings/errors; report: {}",
            report.counts.total_cells,
            report.warning_count(),
            target.display()
        );
        return Ok(());
    }
    // Independent terrain-generation acceptance path, including numeric
    // structure and an actual generated physical relief PNG preview.
    if let Some(index) = args.iter().position(|a| a == "--generate-terrain") {
        let outdir = args
            .get(index + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("maps/generated-terrain"));
        if let Err(error) = project.layers.apply(&mut world) {
            eprintln!("Could not load authored project layers: {error}");
            std::process::exit(2);
        }
        let result = match terrain_generation::generate(&world, &project.terrain_recipe) {
            Ok(generated) => generated,
            Err(error) => {
                eprintln!("Geological terrain generation failed: {error}");
                std::process::exit(2);
            }
        };
        if let Err(error) =
            terrain_generation::export_generation(&result, &project.terrain_recipe, &outdir)
        {
            eprintln!("Could not export generated geology: {error}");
            std::process::exit(2);
        }
        world.elevation = Some(result.elevation);
        let art = Cartography::new(&world);
        if let Err(error) = render::export_atlas_png(
            &world,
            &art,
            Theme::Terrain,
            &outdir.join("generated-physical-relief.png"),
        ) {
            eprintln!("Could not export generated terrain preview: {error}");
            std::process::exit(2);
        }
        println!(
            "Generated {}x{} geological terrain without using source elevation: {}",
            world.width,
            world.height,
            outdir.display()
        );
        return Ok(());
    }
    // Reproducible node graph builds run headlessly, before the wgpu window.
    // The graph consumes exactly the same authored DEM that the editor displays.
    if let Some(index) = args.iter().position(|a| a == "--build-graph") {
        let Some(input) = args.get(index + 1) else {
            eprintln!("Usage: --build-graph RECIPE.json OUTPUT_DIRECTORY");
            std::process::exit(2);
        };
        let outdir = args
            .get(index + 2)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("maps/generated"));
        if let Err(error) = activate_terrain(&mut world, &project) {
            eprintln!("Could not prepare terrain for map graph: {error}");
            std::process::exit(2);
        }
        let graph = match map_graph::MapGraph::from_json(Path::new(input)) {
            Ok(recipe) => recipe,
            Err(error) => {
                eprintln!("Invalid graph recipe: {error}");
                std::process::exit(2);
            }
        };
        let build = match graph.build(&world) {
            Ok(result) => result,
            Err(error) => {
                eprintln!("Map graph build failed: {error}");
                std::process::exit(2);
            }
        };
        match map_graph::export_outputs(&graph, &build, &outdir) {
            Ok(count) => println!(
                "Graph '{}' evaluated {} nodes and exported {} numeric maps to {}",
                graph.name,
                build.evaluated_nodes,
                count,
                outdir.display()
            ),
            Err(error) => {
                eprintln!("Graph export failed: {error}");
                std::process::exit(2);
            }
        }
        return Ok(());
    }
    if let Some(index) = args.iter().position(|a| a == "--export-maps") {
        if let Err(error) = activate_terrain(&mut world, &project) {
            eprintln!("Could not prepare terrain for atlas export: {error}");
            std::process::exit(2);
        }
        let output = args
            .get(index + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("maps"));
        world.slope_deg = world
            .elevation
            .as_ref()
            .and_then(|dem| map_graph::slope_degrees(dem, 6371.0).ok());
        world.rain_shadow = world
            .elevation
            .as_ref()
            .map(|dem| rainshadow::derive(dem, project.wind_from));
        let art = Cartography::new(&world);
        let mut completed = 0;
        for theme in Theme::ALL {
            if !theme.available(&world) {
                continue;
            }
            let target = output.join(format!("{}.png", theme.slug()));
            if let Err(error) = render::export_atlas_png(&world, &art, theme, &target) {
                eprintln!("Failed to export {}: {}", theme.name(), error);
                return Ok(());
            }
            let sheet = output.join(format!("{}-atlas.svg", theme.slug()));
            if let Err(error) =
                publication::export_sheet(&world, &art, theme, &project.name, &sheet)
            {
                eprintln!("Could not export SVG sheet for {}: {}", theme.name(), error);
                return Ok(());
            }
            println!(
                "Exported {} ({}, native {}x{})",
                target.display(),
                theme.units(),
                world.width,
                world.height
            );
            completed += 1;
        }
        println!(
            "{} data-driven maps exported. Source geological data was not modified.",
            completed
        );
        return Ok(());
    }
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1050.0, 680.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Aegaea Studio",
        options,
        Box::new(move |cc| Ok(Box::new(Studio::new(cc, world, project, path, status)))),
    )
}

#[cfg(all(test, target_os = "windows"))]
mod graphics_backend_tests {
    #[test]
    fn windows_build_has_a_native_wgpu_backend() {
        let backends = wgpu::Instance::enabled_backend_features();
        assert!(
            backends.intersects(wgpu::Backends::DX12 | wgpu::Backends::VULKAN),
            "eframe's Wgpu renderer requires a compiled DX12 or Vulkan backend on Windows"
        );
    }
}

#[cfg(test)]
mod map_canvas_tests {
    use super::*;
    #[test]
    fn fill_canvas_crops_map_without_stretching_geography() {
        let uv = atlas_crop_uv(2.0, 1.0, 1.0, egui::pos2(0.5, 0.5));
        assert!((uv.width() - 0.5).abs() < 1e-6);
        assert!((uv.height() - 1.0).abs() < 1e-6);
        let fitted = atlas_crop_uv(2.0, 2.0, 1.0, egui::pos2(0.5, 0.5));
        assert!((fitted.width() - 1.0).abs() < 1e-6);
        assert!((fitted.height() - 1.0).abs() < 1e-6);
    }
    #[test]
    fn map_pan_stays_in_bounds_with_zoom_and_cropping() {
        let uv = atlas_crop_uv(1.8, 1.0, 4.0, egui::pos2(1.5, -3.0));
        assert!(uv.min.x >= 0.0 && uv.max.x <= 1.00001);
        assert!(uv.min.y >= 0.0 && uv.max.y <= 1.00001);
    }
}

#[cfg(test)]
mod terrain_activation_tests {
    use super::*;
    fn fixture() -> World {
        let mut w = World::empty(PathBuf::from("custom"));
        w.width = 24;
        w.height = 12;
        w.elevation = Some(world::Grid::new(24, 12, 8100.0));
        let mut province = world::Grid::new(24, 12, 0.0);
        let mut crust = world::Grid::new(24, 12, 7.0);
        for y in 3..9 {
            for x in 4..19 {
                province.cells[y * 24 + x] = 3.0;
                crust.cells[y * 24 + x] = 35.0;
            }
        }
        w.province_ids = Some(province);
        w.crust_km = Some(crust);
        w.plate_ids = Some(world::Grid::new(24, 12, 0.0));
        w
    }
    #[test]
    fn default_project_uses_generated_terrain_not_imported_elevation() {
        let mut w = fixture();
        let project = Project::new("New");
        assert_eq!(
            activate_terrain(&mut w, &project).unwrap(),
            TerrainMode::Generated
        );
        assert!(w.elevation.as_ref().unwrap().cells.iter().any(|v| *v < 0.0));
        assert!(!w
            .elevation
            .as_ref()
            .unwrap()
            .cells
            .iter()
            .all(|v| *v == 8100.0));
    }
    #[test]
    fn independent_world_reloads_authored_source_layers_before_derivations() {
        let mut author_world = World::empty(PathBuf::from("custom"));
        author_world.width = 48;
        author_world.height = 24;
        let mut project = Project::new("Independent");
        project
            .terrain
            .create_flat_world(&mut author_world, -2000.0)
            .unwrap();
        project.terrain_mode = TerrainMode::Imported;
        for kind in [
            LayerKind::Plates,
            LayerKind::Cratons,
            LayerKind::Provinces,
            LayerKind::Crust,
            LayerKind::Lithosphere,
            LayerKind::HeatFlow,
        ] {
            project
                .layers
                .create(&mut author_world, kind, kind.default_fill())
                .unwrap();
        }
        project.layers.add_craton(&mut author_world).unwrap();
        project
            .layers
            .paint(&mut author_world, LayerKind::Cratons, 30.0, 20.0, 2, 1.0)
            .unwrap();
        project
            .layers
            .paint(&mut author_world, LayerKind::HeatFlow, 30.0, 20.0, 2, 100.0)
            .unwrap();
        let saved = serde_json::to_string(&project).unwrap();
        let restored: Project = serde_json::from_str(&saved).unwrap();
        let mut reload = World::empty(PathBuf::from("custom"));
        reload.width = 48;
        reload.height = 24;
        assert_eq!(
            activate_terrain(&mut reload, &restored).unwrap(),
            TerrainMode::Imported
        );
        assert!(reload
            .elevation
            .as_ref()
            .unwrap()
            .cells
            .iter()
            .all(|&v| v == -2000.0));
        assert_eq!(reload.plates.len(), 1);
        assert_eq!(reload.cratons.len(), 1);
        assert_eq!(reload.provinces.len(), 11);
        assert!(reload
            .heatflow
            .as_ref()
            .unwrap()
            .cells
            .iter()
            .any(|&v| v == 100.0));
        assert!(reload
            .craton_ids
            .as_ref()
            .unwrap()
            .cells
            .iter()
            .any(|&v| v == 1.0));
        assert!(
            reload.plate_motion.is_none(),
            "unknown Euler motion is not fabricated"
        );
    }

    #[test]
    fn explicitly_imported_source_is_preserved() {
        let mut w = fixture();
        let mut project = Project::new("New");
        project.terrain_mode = TerrainMode::Imported;
        assert_eq!(
            activate_terrain(&mut w, &project).unwrap(),
            TerrainMode::Imported
        );
        assert!(w
            .elevation
            .as_ref()
            .unwrap()
            .cells
            .iter()
            .all(|v| *v == 8100.0));
    }
    #[test]
    fn auto_existing_overrides_remain_on_legacy_base() {
        let mut w = fixture();
        let mut project = Project::new("Old project");
        project.terrain.overrides.insert(0, 7000.0);
        assert_eq!(
            activate_terrain(&mut w, &project).unwrap(),
            TerrainMode::Imported
        );
        assert_eq!(w.elevation.as_ref().unwrap().cells[0], 7000.0);
    }
}
