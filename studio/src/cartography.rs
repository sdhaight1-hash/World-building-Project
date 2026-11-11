//! Data-driven cartographic styling over *native* geological fields.
//! No prepainted textures or inferred scientific layers are stored in this module.
use crate::{render::Theme, world::World};
use std::fs;

const NO_BOUNDARY: u8 = 255;
const PLATE_FALLBACK: [[u8; 3]; 17] = [
    [121, 161, 184],
    [228, 168, 105],
    [88, 172, 166],
    [202, 134, 121],
    [161, 145, 203],
    [98, 171, 209],
    [229, 193, 121],
    [139, 178, 123],
    [92, 163, 189],
    [185, 135, 174],
    [191, 117, 127],
    [186, 177, 105],
    [113, 143, 188],
    [133, 183, 139],
    [202, 149, 162],
    [132, 180, 186],
    [180, 164, 145],
];
const PROVINCE_COLORS: [[u8; 3]; 11] = [
    [44, 81, 118],   // ocean basin
    [71, 161, 177],  // ocean spreading
    [52, 46, 105],   // trench
    [169, 157, 134], // continental crust
    [196, 113, 115], // exposed cratonic shield
    [211, 174, 125], // cratonic platform
    [153, 122, 175], // ancient mobile belt
    [175, 95, 85],   // orogen
    [204, 121, 73],  // volcanic arc
    [203, 159, 88],  // rift
    [136, 109, 139], // transform shear zone
];

#[derive(Clone)]
pub struct LegendItem {
    pub label: String,
    pub rgb: [u8; 3],
}

#[derive(Default, Debug)]
pub struct Cartography {
    width: usize,
    height: usize,
    relief: Vec<f32>,
    coast: Vec<bool>,
    plate_edge: Vec<bool>,
    craton_edge: Vec<bool>,
    province_edge: Vec<bool>,
    contact_class: Vec<u8>,
    pub classified_segments: usize,
}

fn blend(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let c = |i| ((a[i] as f32) * (1.0 - t) + (b[i] as f32) * t).round() as u8;
    [c(0), c(1), c(2)]
}

fn scale(color: [u8; 3], value: f32) -> [u8; 3] {
    let f = value.clamp(0.35, 1.30);
    color.map(|channel| ((channel as f32) * f).clamp(0.0, 255.0) as u8)
}

fn gradient(value: f32, breaks: &[(f32, [u8; 3])]) -> [u8; 3] {
    if !value.is_finite() {
        return [89, 99, 111];
    }
    if value <= breaks[0].0 {
        return breaks[0].1;
    }
    for pair in breaks.windows(2) {
        if value <= pair[1].0 {
            let t = (value - pair[0].0) / (pair[1].0 - pair[0].0);
            return blend(pair[0].1, pair[1].1, t);
        }
    }
    breaks[breaks.len() - 1].1
}

pub fn terrain_color(m: f32) -> [u8; 3] {
    // Physical DEM in metres: explicit separate ocean-depth and land-elevation ramps.
    if m < 0.0 {
        gradient(
            m,
            &[
                (-9000.0, [10, 29, 60]),
                (-6000.0, [18, 47, 94]),
                (-4000.0, [26, 81, 135]),
                (-2000.0, [35, 113, 155]),
                (-200.0, [70, 151, 178]),
                (0.0, [116, 183, 191]),
            ],
        )
    } else {
        gradient(
            m,
            &[
                (0.0, [64, 118, 85]),
                (250.0, [111, 151, 101]),
                (800.0, [157, 170, 117]),
                (1600.0, [185, 160, 111]),
                (2800.0, [160, 134, 110]),
                (4200.0, [145, 136, 135]),
                (5500.0, [197, 195, 190]),
                (7000.0, [245, 243, 233]),
            ],
        )
    }
}

fn discrete_color(id: usize) -> [u8; 3] {
    PLATE_FALLBACK[id % PLATE_FALLBACK.len()]
}

fn plate_color(w: &World, id: usize) -> [u8; 3] {
    w.plates
        .iter()
        .find(|p| usize::from(p.plate_id) == id)
        .and_then(|p| parse_rgb(&p.color_rgb))
        .unwrap_or_else(|| discrete_color(id))
}

fn parse_rgb(raw: &str) -> Option<[u8; 3]> {
    let mut fields = raw.split(',').map(|x| x.trim().parse::<u8>().ok());
    Some([fields.next()??, fields.next()??, fields.next()??])
}

/// Velocity-component styling, in cm/year. A subdued zero field keeps the
/// planet visible while active contacts carry high-contrast scientific data.
fn motion_color(theme: Theme, speed: f32) -> [u8; 3] {
    let speed = speed.max(0.0);
    if speed < 0.1 {
        return [51, 61, 75];
    }
    let ramp: &[(f32, [u8; 3])] = match theme {
        Theme::PlateConvergence => &[
            (0.0, [82, 88, 100]),
            (1.0, [135, 93, 96]),
            (3.0, [199, 106, 85]),
            (6.0, [242, 150, 89]),
            (10.0, [255, 218, 142]),
        ],
        Theme::PlateDivergence => &[
            (0.0, [68, 90, 97]),
            (1.0, [56, 128, 151]),
            (3.0, [64, 175, 174]),
            (6.0, [93, 214, 188]),
            (10.0, [177, 245, 203]),
        ],
        Theme::PlateShear => &[
            (0.0, [73, 79, 99]),
            (1.0, [100, 103, 171]),
            (3.0, [149, 111, 201]),
            (6.0, [199, 143, 235]),
            (10.0, [245, 205, 255]),
        ],
        _ => return [51, 61, 75],
    };
    gradient(speed, ramp)
}

fn contact_color(id: u8) -> [u8; 3] {
    match id {
        0 => [240, 89, 81],   // subduction
        1 => [238, 169, 82],  // continental collision
        2 => [83, 224, 207],  // spreading/divergence
        3 => [191, 125, 236], // transform
        _ => [226, 234, 238],
    }
}

impl Cartography {
    pub fn new(w: &World) -> Self {
        let width = w.width;
        let height = w.height;
        let len = width * height;
        let mut art = Self {
            width,
            height,
            relief: vec![1.0; len],
            coast: vec![false; len],
            plate_edge: vec![false; len],
            craton_edge: vec![false; len],
            province_edge: vec![false; len],
            contact_class: vec![NO_BOUNDARY; len],
            classified_segments: 0,
        };
        if let Some(e) = &w.elevation {
            let sphere_radius_m = 6_371_000.0_f64;
            let spacing_y = std::f64::consts::PI * sphere_radius_m / height as f64;
            for y in 0..height {
                let lat = -std::f64::consts::FRAC_PI_2
                    + std::f64::consts::PI * ((y as f64 + 0.5) / (height as f64));
                let spacing_x = (std::f64::consts::TAU * sphere_radius_m * lat.cos().abs()
                    / width as f64)
                    .max(1500.0);
                for x in 0..width {
                    let id = y * width + x;
                    let height_here = e.at(x, y);
                    let east = e.at((x + width - 1) % width, y);
                    let west = e.at((x + 1) % width, y);
                    let north = e.at(x, (y + 1).min(height - 1));
                    let south = e.at(x, y.saturating_sub(1));
                    let dh_e = (east - west) as f64 / (2.0 * spacing_x);
                    let dh_n = (north - south) as f64 / (2.0 * spacing_y);
                    // Exaggerated *lighting* only; source metre elevations remain untouched.
                    let (nx, ny, nz) = (-dh_e * 11.0, -dh_n * 11.0, 1.0);
                    let norm = (nx * nx + ny * ny + nz * nz).sqrt();
                    let sun = (nx * 0.52 + ny * 0.48 + nz * 0.71) / norm;
                    art.relief[id] = (0.58 + 0.46 * sun.max(-0.8)) as f32;
                    art.coast[id] = [east, west, north, south]
                        .iter()
                        .any(|other| (*other < 0.0) != (height_here < 0.0));
                }
            }
        }
        for (kind, (grid, mask)) in [
            (&w.plate_ids, &mut art.plate_edge),
            (&w.craton_ids, &mut art.craton_edge),
            (&w.province_ids, &mut art.province_edge),
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(g) = grid {
                for y in 0..height {
                    for x in 0..width {
                        let id = g.at(x, y) as usize;
                        // A craton ID of zero is background, not a boundary to highlight.
                        if kind == 1 && id == 0 {
                            continue;
                        }
                        mask[y * width + x] = id != g.at((x + 1) % width, y) as usize
                            || (y + 1 < height && id != g.at(x, y + 1) as usize);
                    }
                }
            }
        }
        art.load_boundary_segments(w);
        for segment in &w.boundary_segments {
            if segment.validate().is_err() {
                continue;
            }
            let xy = |point: [f64; 2]| {
                (
                    (180.0 - point[0]) / 360.0 * width as f64,
                    (point[1] + 90.0) / 180.0 * height as f64,
                )
            };
            art.line(xy(segment.start), xy(segment.end), segment.class);
            art.classified_segments += 1;
        }
        art
    }

    fn load_boundary_segments(&mut self, w: &World) {
        let path = w.root.join("boundary_segments.geojson");
        let Ok(contents) = fs::read_to_string(&path) else {
            return;
        };
        let Ok(file) = serde_json::from_str::<serde_json::Value>(&contents) else {
            return;
        };
        let Some(features) = file.get("features").and_then(|v| v.as_array()) else {
            return;
        };
        for feature in features {
            let class = match feature["properties"]["type"].as_str() {
                Some("subduction") => 0,
                Some("continental_collision") => 1,
                Some("divergent_spreading") => 2,
                Some("transform") => 3,
                _ => continue,
            };
            let Some(coords) = feature["geometry"]["coordinates"].as_array() else {
                continue;
            };
            let mut previous = None;
            for coord in coords {
                let (Some(lon), Some(lat)) = (
                    coord.get(0).and_then(|v| v.as_f64()),
                    coord.get(1).and_then(|v| v.as_f64()),
                ) else {
                    continue;
                };
                let pixel = (
                    (180.0 - lon) / 360.0 * self.width as f64,
                    (lat + 90.0) / 180.0 * self.height as f64,
                );
                if let Some(p) = previous {
                    self.line(p, pixel, class);
                }
                previous = Some(pixel);
            }
            self.classified_segments += 1;
        }
    }

    fn line(&mut self, a: (f64, f64), mut b: (f64, f64), class: u8) {
        let w = self.width as f64;
        if b.0 - a.0 > w / 2.0 {
            b.0 -= w;
        }
        if a.0 - b.0 > w / 2.0 {
            b.0 += w;
        }
        let dx = b.0 - a.0;
        let dy = b.1 - a.1;
        let steps = ((dx.abs().max(dy.abs()) * 2.0).ceil() as usize).max(1);
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let x = (a.0 + dx * t).round() as isize;
            let y = (a.1 + dy * t).round() as isize;
            // A 2-pixel classified line remains readable in native atlas exports.
            for (px, py) in [(x, y), (x + 1, y), (x, y + 1)] {
                if py < 0 || py >= self.height as isize {
                    continue;
                }
                let xx = px.rem_euclid(self.width as isize) as usize;
                self.contact_class[py as usize * self.width + xx] = class;
            }
        }
    }

    pub fn sample(&self, w: &World, theme: Theme, x: usize, y: usize) -> [u8; 3] {
        let index = y * self.width + x;
        let elev = w.elevation.as_ref().map(|g| g.at(x, y));
        let base = elev.map(terrain_color).unwrap_or([54, 66, 79]);
        let relief = self.relief[index];
        let base_relief = scale(base, relief);
        let mut fill = match theme {
            Theme::Terrain => base_relief,
            Theme::Slope => {
                if let Some(ref g) = w.slope_deg {
                    let tint = gradient(
                        g.at(x, y),
                        &[
                            (0.0, [94, 142, 132]),
                            (2.0, [131, 177, 134]),
                            (5.0, [197, 187, 117]),
                            (15.0, [223, 144, 83]),
                            (30.0, [199, 93, 76]),
                            (50.0, [127, 53, 71]),
                            (90.0, [69, 37, 61]),
                        ],
                    );
                    scale(blend(base, tint, 0.93), relief)
                } else {
                    base_relief
                }
            }
            Theme::Plates | Theme::Boundaries => {
                if theme == Theme::Boundaries {
                    scale(blend(base, [94, 106, 114], 0.50), relief)
                } else if let Some(ref g) = w.plate_ids {
                    let color = plate_color(w, g.at(x, y) as usize);
                    scale(blend(base, color, 0.82), relief)
                } else {
                    base_relief
                }
            }
            Theme::PlateConvergence | Theme::PlateDivergence | Theme::PlateShear => {
                if let Some(ref motion) = w.plate_motion {
                    let value = match theme {
                        Theme::PlateConvergence => motion.convergence_cm_yr.at(x, y),
                        Theme::PlateDivergence => motion.divergence_cm_yr.at(x, y),
                        Theme::PlateShear => motion.shear_cm_yr.at(x, y),
                        _ => unreachable!(),
                    };
                    let tint = motion_color(theme, value);
                    // No second hillshade pass: velocities should remain legible.
                    blend(base, tint, if value < 0.1 { 0.92 } else { 0.98 })
                } else {
                    base_relief
                }
            }
            Theme::Cratons => {
                if let Some(ref g) = w.craton_ids {
                    let id = g.at(x, y) as usize;
                    if id == 0 {
                        scale(blend(base, [105, 111, 116], 0.37), relief)
                    } else {
                        scale(blend(base, discrete_color(id + 4), 0.86), relief)
                    }
                } else {
                    base_relief
                }
            }
            Theme::BasementAge => {
                if let Some(ref g) = w.craton_ids {
                    let id = g.at(x, y) as u8;
                    let age = w
                        .cratons
                        .iter()
                        .find(|c| c.craton_id == id)
                        .map(|c| c.basement_age_ga);
                    if let Some(age) = age {
                        scale(
                            blend(
                                base,
                                gradient(
                                    age,
                                    &[
                                        (2.4, [227, 202, 122]),
                                        (2.8, [219, 146, 107]),
                                        (3.1, [176, 106, 143]),
                                        (3.5, [115, 93, 176]),
                                    ],
                                ),
                                0.90,
                            ),
                            relief,
                        )
                    } else {
                        scale(blend(base, [127, 135, 136], 0.43), relief)
                    }
                } else {
                    base_relief
                }
            }
            Theme::Provinces => {
                if let Some(ref g) = w.province_ids {
                    let id = g.at(x, y) as usize;
                    let tint = if id < PROVINCE_COLORS.len() {
                        PROVINCE_COLORS[id]
                    } else {
                        discrete_color(id)
                    };
                    scale(blend(base, tint, 0.90), relief)
                } else {
                    base_relief
                }
            }
            Theme::Crust => {
                if let Some(ref g) = w.crust_km {
                    let tint = gradient(
                        g.at(x, y),
                        &[
                            (0.0, [41, 102, 154]),
                            (10.0, [56, 159, 169]),
                            (25.0, [136, 192, 149]),
                            (40.0, [227, 191, 107]),
                            (70.0, [192, 89, 91]),
                        ],
                    );
                    scale(blend(base, tint, 0.92), relief)
                } else {
                    base_relief
                }
            }
            Theme::Lithosphere => {
                if let Some(ref g) = w.lithosphere_km {
                    let tint = gradient(
                        g.at(x, y),
                        &[
                            (0.0, [33, 97, 160]),
                            (50.0, [76, 163, 183]),
                            (110.0, [134, 195, 157]),
                            (170.0, [224, 187, 115]),
                            (260.0, [167, 98, 135]),
                        ],
                    );
                    scale(blend(base, tint, 0.92), relief)
                } else {
                    base_relief
                }
            }
            Theme::HeatFlow => {
                if let Some(ref g) = w.heatflow {
                    let tint = gradient(
                        g.at(x, y),
                        &[
                            (20.0, [47, 104, 169]),
                            (40.0, [73, 153, 187]),
                            (60.0, [163, 199, 151]),
                            (90.0, [239, 181, 97]),
                            (150.0, [196, 70, 70]),
                        ],
                    );
                    scale(blend(base, tint, 0.91), relief)
                } else {
                    base_relief
                }
            }
            Theme::MonthlyTemperature | Theme::MonthlyPrecipitation => {
                if let Some(field) = theme.climate_field() {
                    if let Some(grid) = w.scientific_fields.get(field.id()) {
                        let (stops, underlay): (&[(f32, [u8; 3])], f32) = match theme {
                            Theme::MonthlyTemperature => (
                                &[
                                    (-60.0, [45, 57, 116]),
                                    (-25.0, [77, 123, 173]),
                                    (0.0, [135, 198, 192]),
                                    (10.0, [176, 201, 123]),
                                    (20.0, [237, 193, 91]),
                                    (30.0, [225, 123, 76]),
                                    (45.0, [134, 56, 80]),
                                ],
                                0.08,
                            ),
                            Theme::MonthlyPrecipitation => (
                                &[
                                    (0.0, [188, 147, 101]),
                                    (25.0, [207, 187, 134]),
                                    (60.0, [173, 197, 153]),
                                    (120.0, [98, 181, 156]),
                                    (250.0, [58, 142, 168]),
                                    (500.0, [47, 92, 165]),
                                    (1000.0, [70, 54, 125]),
                                ],
                                0.06,
                            ),
                            _ => unreachable!(),
                        };
                        blend(gradient(grid.at(x, y), stops), base, underlay)
                    } else {
                        [54, 66, 79]
                    }
                } else {
                    [54, 66, 79]
                }
            }
            Theme::RainShadow => {
                if let Some(ref g) = w.rain_shadow {
                    let value = g.at(x, y);
                    if !value.is_finite() {
                        return [21, 64, 94];
                    }
                    let neutral = [172, 181, 175];
                    let tint = if value < 0.0 {
                        blend(neutral, [47, 140, 190], -value)
                    } else {
                        blend(neutral, [218, 92, 52], value)
                    };
                    scale(blend(base, tint, 0.93), relief)
                } else {
                    base_relief
                }
            }
        };
        if self.coast[index] && theme != Theme::RainShadow {
            // Avoid the heavy pixel-wide black shoreline on the native DEM.
            fill = blend(fill, [27, 62, 75], 0.29);
        }
        match theme {
            // Suppress the prior one-sided 1,000-m cell threshold 'contours'.
            // They formed an aliased stipple, not actual contour isolines.
            // Proper marching-squares vector contour rendering is a later phase.
            Theme::Terrain => {}
            Theme::Plates | Theme::Boundaries => {
                if self.plate_edge[index] {
                    fill = blend(fill, [18, 28, 38], 0.64);
                }
                let class = self.contact_class[index];
                if class != NO_BOUNDARY {
                    fill = contact_color(class);
                }
            }
            Theme::Cratons | Theme::BasementAge => {
                if self.craton_edge[index] {
                    fill = blend(fill, [32, 33, 43], 0.82);
                }
            }
            Theme::Provinces => {
                if self.province_edge[index] {
                    fill = blend(fill, [24, 33, 43], 0.54);
                }
            }
            _ => {}
        }
        fill
    }

    pub fn legend(&self, w: &World, theme: Theme) -> Vec<LegendItem> {
        let items: Vec<(String, [u8; 3])> = match theme {
            Theme::Slope => vec![
                ("0° · level".into(), [94, 142, 132]),
                ("2° · nearly level".into(), [131, 177, 134]),
                ("5° · gentle".into(), [197, 187, 117]),
                ("15° · moderate".into(), [223, 144, 83]),
                ("30° · steep".into(), [199, 93, 76]),
                ("50° · very steep".into(), [127, 53, 71]),
            ],
            Theme::Terrain => vec![
                ("Ocean abyss · < -6000 m".into(), terrain_color(-6500.0)),
                ("Continental slope · -2000 m".into(), terrain_color(-2000.0)),
                ("Shallow seas · -200 m".into(), terrain_color(-200.0)),
                ("Lowlands · 250 m".into(), terrain_color(250.0)),
                ("Highlands · 1600 m".into(), terrain_color(1600.0)),
                ("Mountains · 4200 m".into(), terrain_color(4200.0)),
                ("High peaks · 6000 m".into(), terrain_color(6000.0)),
            ],
            Theme::Plates => w
                .plates
                .iter()
                .map(|p| {
                    (
                        p.plate.clone(),
                        parse_rgb(&p.color_rgb).unwrap_or(discrete_color(p.plate_id as usize)),
                    )
                })
                .collect(),
            Theme::Boundaries if self.classified_segments == 0 => {
                vec![("Unclassified plate contact".into(), [18, 28, 38])]
            }
            Theme::Boundaries => vec![
                ("Subduction".into(), contact_color(0)),
                ("Continental collision".into(), contact_color(1)),
                ("Divergent spreading".into(), contact_color(2)),
                ("Transform".into(), contact_color(3)),
            ],
            Theme::PlateConvergence | Theme::PlateDivergence | Theme::PlateShear => {
                [0.0f32, 1.0, 3.0, 6.0, 10.0]
                    .iter()
                    .map(|&value| {
                        (
                            if value == 0.0 {
                                "0 · no modeled component".to_owned()
                            } else {
                                format!("{value:.0} cm/year")
                            },
                            motion_color(theme, value),
                        )
                    })
                    .collect()
            }
            Theme::Cratons => w
                .cratons
                .iter()
                .map(|c| (c.name.clone(), discrete_color(c.craton_id as usize + 4)))
                .collect(),
            Theme::BasementAge => vec![
                ("2.4 Ga · younger".into(), [227, 202, 122]),
                ("2.8 Ga".into(), [219, 146, 107]),
                ("3.1 Ga".into(), [176, 106, 143]),
                ("3.5 Ga · older".into(), [115, 93, 176]),
            ],
            Theme::Provinces => w
                .provinces
                .iter()
                .map(|p| {
                    (
                        p.name.replace('_', " "),
                        PROVINCE_COLORS
                            .get(p.province_id as usize)
                            .copied()
                            .unwrap_or_else(|| discrete_color(p.province_id as usize)),
                    )
                })
                .collect(),
            Theme::Crust => vec![
                ("0–10 km".into(), [41, 102, 154]),
                ("25 km".into(), [136, 192, 149]),
                ("40 km".into(), [227, 191, 107]),
                ("70 km".into(), [192, 89, 91]),
            ],
            Theme::Lithosphere => vec![
                ("0–50 km".into(), [33, 97, 160]),
                ("110 km".into(), [134, 195, 157]),
                ("170 km".into(), [224, 187, 115]),
                ("260 km".into(), [167, 98, 135]),
            ],
            Theme::HeatFlow => vec![
                ("20 mW/m²".into(), [47, 104, 169]),
                ("60 mW/m²".into(), [163, 199, 151]),
                ("90 mW/m²".into(), [239, 181, 97]),
                ("150 mW/m²".into(), [196, 70, 70]),
            ],
            Theme::RainShadow => vec![
                ("Windward · negative index".into(), [47, 140, 190]),
                ("Neutral · near zero".into(), [172, 181, 175]),
                ("Leeward · positive index".into(), [218, 92, 52]),
            ],
            Theme::MonthlyTemperature => [-60.0_f32, -25.0, 0.0, 10.0, 20.0, 30.0, 45.0]
                .iter()
                .map(|&value| (
                    format!("{value:.0} °C"),
                    gradient(value, &[
                        (-60.0, [45, 57, 116]),
                        (-25.0, [77, 123, 173]),
                        (0.0, [135, 198, 192]),
                        (10.0, [176, 201, 123]),
                        (20.0, [237, 193, 91]),
                        (30.0, [225, 123, 76]),
                        (45.0, [134, 56, 80]),
                    ]),
                ))
                .collect(),
            Theme::MonthlyPrecipitation => [0.0_f32, 25.0, 60.0, 120.0, 250.0, 500.0, 1000.0]
                .iter()
                .map(|&value| (
                    format!("{value:.0} mm/month"),
                    gradient(value, &[
                        (0.0, [188, 147, 101]),
                        (25.0, [207, 187, 134]),
                        (60.0, [173, 197, 153]),
                        (120.0, [98, 181, 156]),
                        (250.0, [58, 142, 168]),
                        (500.0, [47, 92, 165]),
                        (1000.0, [70, 54, 125]),
                    ]),
                ))
                .collect(),

        };
        items
            .into_iter()
            .map(|(label, rgb)| LegendItem { label, rgb })
            .collect()
    }

    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Grid;
    use std::path::PathBuf;

    #[test]
    fn terrain_ramp_preserves_elevation_ordering() {
        assert_ne!(terrain_color(-8000.0), terrain_color(-2000.0));
        assert_ne!(terrain_color(500.0), terrain_color(4200.0));
    }

    #[test]
    fn actual_reference_boundaries_and_layers_are_used() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/aegaea/tectonics/v1");
        let world = World::load(root).expect("reference import");
        let cart = Cartography::new(&world);
        assert!(
            cart.classified_segments > 4500,
            "must load source boundary linework"
        );
        assert_eq!(cart.size(), (1672, 941));
        assert_eq!(cart.legend(&world, Theme::Plates).len(), 17);
        assert_eq!(cart.legend(&world, Theme::Provinces).len(), 11);
        assert!(cart.contact_class.iter().any(|&x| x == 0));
        assert!(cart.contact_class.iter().any(|&x| x == 2));
    }

    #[test]
    fn manual_classification_is_visible_but_not_inferred_from_plate_edges() {
        let mut world = World::empty(PathBuf::from("blank-project"));
        world.width = 64;
        world.height = 32;
        let mut ids = Grid::new(64, 32, 1.0);
        for y in 0..32 {
            for x in 0..32 {
                ids.cells[y * 64 + x] = 2.0;
            }
        }
        world.plate_ids = Some(ids);
        let unclassified = Cartography::new(&world);
        assert_eq!(unclassified.classified_segments, 0);
        assert_eq!(
            unclassified.legend(&world, Theme::Boundaries)[0].label,
            "Unclassified plate contact"
        );

        world
            .boundary_segments
            .push(crate::map_layers::BoundarySegment {
                start: [0.0, 0.0],
                end: [25.0, 0.0],
                class: 3,
            });
        let authored = Cartography::new(&world);
        assert_eq!(authored.classified_segments, 1);
        assert_eq!(
            authored.sample(&world, Theme::Boundaries, 32, 16),
            contact_color(3)
        );
        assert!(authored
            .legend(&world, Theme::Boundaries)
            .iter()
            .any(|l| l.label == "Transform"));
    }

    #[test]
    fn classified_boundaries_use_real_source_geometry() {
        let mut world = World::empty(PathBuf::new());
        world.width = 4;
        world.height = 3;
        world.elevation = Some(Grid::new(4, 3, 500.0));
        let map = Cartography::new(&world);
        assert!(map.contact_class.iter().all(|&v| v == NO_BOUNDARY));
    }
}
