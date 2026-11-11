//! Aegaea Studio P0: a modern wgpu-backed desktop shell around physical-world maps.
//! Purposefully scoped: shared atlas/globe source data, inspector, local point annotation.
mod cartography;
mod rainshadow;
mod render;
mod world;

use cartography::Cartography;
use eframe::{
    egui,
    egui::{Color32, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2},
};
use rainshadow::WindFrom;
use render::{atlas_geo_from_fraction, forward_globe, globe_geo_from_fraction, Theme};
use std::{
    env,
    path::{Path, PathBuf},
};
use world::{geo_to_uv, Project, World};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Atlas,
    Globe,
}

struct Studio {
    world: World,
    cartography: Cartography,
    project: Project,
    project_path: PathBuf,
    theme: Theme,
    view: ViewMode,
    atlas: Option<TextureHandle>,
    globe: Option<TextureHandle>,
    atlas_dirty: bool,
    globe_dirty: bool,
    globe_lon: f64,
    globe_lat: f64,
    zoom: f32,
    pan_u: f32,
    pan_v: f32,
    picked: Option<(f64, f64)>,
    selected_marker: Option<usize>,
    placing_marker: bool,
    changed: bool,
    status: String,
}

impl Studio {
    fn new(
        cc: &eframe::CreationContext<'_>,
        mut world: World,
        project: Project,
        path: PathBuf,
        status: String,
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
        // Compute once from the restored/source DEM; never alter source geology.
        world.rain_shadow = world
            .elevation
            .as_ref()
            .map(|dem| rainshadow::derive(dem, project.wind_from));
        let cartography = Cartography::new(&world);
        let mut app = Self {
            world,
            cartography,
            project,
            project_path: path,
            theme: Theme::Terrain,
            view: ViewMode::Atlas,
            atlas: None,
            globe: None,
            atlas_dirty: true,
            globe_dirty: true,
            globe_lon: 0.0,
            globe_lat: 0.0,
            zoom: 1.0,
            pan_u: 0.5,
            pan_v: 0.5,
            picked: None,
            selected_marker: None,
            placing_marker: false,
            changed: false,
            status,
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
            self.atlas = Some(ctx.load_texture(
                "world-atlas",
                render::atlas_image(&self.world, &self.cartography, self.theme),
                egui::TextureOptions::LINEAR,
            ));
            self.atlas_dirty = false;
        }
        if self.globe_dirty && self.view == ViewMode::Globe {
            self.globe = Some(ctx.load_texture(
                "world-globe",
                render::globe_image(
                    &self.world,
                    &self.cartography,
                    self.theme,
                    self.globe_lon,
                    self.globe_lat,
                ),
                egui::TextureOptions::LINEAR,
            ));
            self.globe_dirty = false;
        }
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
                }
                if ui.button("Save").clicked() {
                    self.save();
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
        egui::SidePanel::left("layers").default_width(225.0).min_width(185.0).show(ctx, |ui| {
            ui.heading("Map library");
            ui.label(egui::RichText::new("THEMATIC VIEW").small().weak());
            ui.add_space(8.0);
            egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
            for theme in Theme::ALL {
                let enabled=theme.available(&self.world);
                let text=if enabled { theme.name().to_string() } else { format!("{}  · unavailable",theme.name()) };
                if ui.add_enabled(enabled,egui::SelectableLabel::new(self.theme==theme,text)).clicked() {
                    self.theme=theme;
                    self.atlas_dirty=true;
                    self.globe_dirty=true;
                }
            }
            });
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
            ui.label(egui::RichText::new("VIEW CONTROLS").small().weak());
            if self.view==ViewMode::Atlas {
                ui.label("Zoom");
                ui.add(egui::Slider::new(&mut self.zoom,1.0..=5.0).show_value(false));
                ui.small("Drag to pan. Scroll or zoom controls will be expanded in the production editor.");
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
            }
            ui.separator();
            ui.label(egui::RichText::new("DATASET").small().weak());
            ui.small(format!("{} × {} native cells", self.world.width,self.world.height));
            ui.small(format!("{} plates · {} cratons",self.world.plates.len(),self.world.cratons.len()));
            if self.world.elevation.is_none() && self.world.has_any_source() {
                ui.colored_label(Color32::from_rgb(230,170,112),
                    "Elevation absent. Rain-shadow map requires a valid DEM.");
            }
            if !self.world.has_any_source() {
                ui.small("Blank world. Import geospatial layers to develop this project.");
            }
            ui.separator();
            ui.label(egui::RichText::new("DATA ETHICS").small().weak());
            ui.small("Geological values are fictional scenario data, not measured Earth observations.");
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

    fn current_uv(&self) -> Rect {
        let span = 1.0 / self.zoom;
        let half = span * 0.5;
        let u = self.pan_u.clamp(half, 1.0 - half);
        let v = self.pan_v.clamp(half, 1.0 - half);
        Rect::from_min_max(
            egui::pos2(u - half, v - half),
            egui::pos2(u + half, v + half),
        )
    }

    fn canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let available = ui.available_size();
            let ratio = if self.view == ViewMode::Globe {
                1.0
            } else {
                self.world.width as f32 / self.world.height as f32
            };
            let width = available.x.min(available.y * ratio).max(5.0);
            let height = width / ratio;
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(format!("{}  ·  {}", self.project.name, self.theme.name()))
                        .strong(),
                );
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(width, (height - 30.0).max(5.0)),
                    Sense::click_and_drag(),
                );
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 4.0, Color32::from_rgb(17, 23, 33));
                let uv = self.current_uv();
                match self.view {
                    ViewMode::Atlas => {
                        if let Some(ref image) = self.atlas {
                            painter.image(image.id(), rect, uv, Color32::WHITE);
                        }
                    }
                    ViewMode::Globe => {
                        if let Some(ref image) = self.globe {
                            painter.image(
                                image.id(),
                                rect,
                                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
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
                                    2.0
                                } else {
                                    1.0
                                },
                                Color32::WHITE,
                            ),
                        );
                    }
                }
                if self.view == ViewMode::Globe && response.dragged() {
                    let delta = ui.input(|input| input.pointer.delta());
                    if delta != Vec2::ZERO {
                        self.globe_lon = (self.globe_lon - (delta.x as f64) * 0.45 + 180.0)
                            .rem_euclid(360.0)
                            - 180.0;
                        self.globe_lat =
                            (self.globe_lat + (delta.y as f64) * 0.45).clamp(-85.0, 85.0);
                        self.globe_dirty = true;
                    }
                } else if self.view == ViewMode::Atlas && response.dragged() {
                    let delta = ui.input(|input| input.pointer.delta());
                    if delta != Vec2::ZERO {
                        self.pan_u -= delta.x / rect.width() / self.zoom;
                        self.pan_v -= delta.y / rect.height() / self.zoom;
                    }
                }
                if response.clicked() {
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
                            if self.placing_marker {
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
        self.topbar(ctx);
        self.layers_panel(ctx);
        self.inspector(ctx);
        self.refresh_textures(ctx);
        self.canvas(ctx);
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
                    "Source geology loaded (fictional scenario)".to_owned()
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
    if let Some(index) = args.iter().position(|a| a == "--export-maps") {
        let output = args
            .get(index + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("maps"));
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
