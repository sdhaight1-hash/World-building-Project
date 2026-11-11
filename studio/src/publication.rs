//! Self-contained, scalable publication sheets for fictional planetary maps.
//!
//! This is NOT a numeric GeoTIFF: the native-resolution PNG map is embedded in
//! an SVG sheet with vector labels, legend, grid, scale note and provenance.
//! SVG is deliberately used so publication layouts remain scalable and
//! inspectable without requiring a headless GPU or bundling font files.
use crate::{cartography::Cartography, render::{self, Theme}, world::World};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use std::{fmt::Write as _, path::Path};

fn escape_xml(value: &str) -> String {
    value.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

fn longitude_name(value: i32) -> String {
    if value > 0 {format!("{value}°E")}
    else if value < 0 {format!("{}°W", -value)}
    else {"0°".into()}
}
fn latitude_name(value: i32) -> String {
    if value > 0 {format!("{value}°N")}
    else if value < 0 {format!("{}°S", -value)}
    else {"0°".into()}
}

/// Export a single self-contained SVG map sheet. The source map remains
/// exactly as rendered in the editor; vector art appears only as a page
/// composition, not as a scientific modification of the dataset.
pub fn export_sheet(
    world: &World,
    cartography: &Cartography,
    theme: Theme,
    world_name: &str,
    path: &Path,
) -> Result<(), String> {
    let image = render::atlas_image(world, cartography, theme);
    let mut rgba = Vec::with_capacity(image.size[0] * image.size[1] * 4);
    for pixel in &image.pixels {
        rgba.extend_from_slice(&pixel.to_array());
    }
    let mut png = Vec::new();
    PngEncoder::new(&mut png).write_image(
        &rgba, image.size[0] as u32, image.size[1] as u32, ColorType::Rgba8.into()
    ).map_err(|e| format!("Could not encode map for publication: {e}"))?;
    let png_b64 = BASE64.encode(&png);

    const PAGE_W: f64 = 2400.0;
    const PAGE_H: f64 = 1530.0;
    const MAP_X: f64 = 120.0;
    const MAP_Y: f64 = 230.0;
    const MAP_W: f64 = 1620.0;
    let map_h = MAP_W * image.size[1] as f64 / image.size[0] as f64;
    let map_bottom = MAP_Y + map_h;
    let key_x = 1820.0;

    let heading = escape_xml(world_name);
    let title = escape_xml(theme.name());
    let units = escape_xml(theme.units());
    let legend = cartography.legend(world, theme);
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{PAGE_W}" height="{PAGE_H}" viewBox="0 0 {PAGE_W} {PAGE_H}" role="img" aria-label="{heading}: {title}">
<rect width="{PAGE_W}" height="{PAGE_H}" fill="#0c1724"/>
<path d="M 120 96 H 2280" stroke="#3f5a70" stroke-width="2"/>
<text x="120" y="73" fill="#90bfcb" font-family="Segoe UI,Arial,sans-serif" font-size="25" font-weight="700" letter-spacing="6">AEGAEA STUDIO · PHYSICAL WORLD ATLAS</text>
<text x="120" y="163" fill="#f1f2ed" font-family="Segoe UI,Arial,sans-serif" font-size="66" font-weight="700">{title}</text>
<text x="2280" y="162" text-anchor="end" fill="#a6b9c5" font-family="Segoe UI,Arial,sans-serif" font-size="30">{heading}</text>
<rect x="{MAP_X}" y="{MAP_Y}" width="{MAP_W}" height="{map_h}" fill="#1b3247" stroke="#8ea4b7" stroke-width="2"/>
<image x="{MAP_X}" y="{MAP_Y}" width="{MAP_W}" height="{map_h}" href="data:image/png;base64,{png_b64}"/>
"##
    );

    // Synthetic atlas: the top is the SOUTH pole and longitude decreases to
    // the right. The grid must reflect that orientation, not Earth's north-up.
    for lon in (-150..=150).step_by(30) {
        let grid_x = MAP_X + (180.0 - lon as f64) / 360.0 * MAP_W;
        let _ = writeln!(
            svg,
            r##"<path d="M {grid_x:.2} {MAP_Y:.2} V {map_bottom:.2}" stroke="#f6f3e7" stroke-opacity=".22" stroke-width="1" stroke-dasharray="7 8"/>
<text x="{grid_x:.2}" y="{:.2}" fill="#c8dae2" font-family="Segoe UI,Arial,sans-serif" font-size="17" text-anchor="middle">{}</text>"##,
            MAP_Y - 9.0, longitude_name(lon)
        );
    }
    for lat in (-60..=60).step_by(30) {
        let grid_y = MAP_Y + (lat as f64 + 90.0) / 180.0 * map_h;
        let _ = writeln!(
            svg,
            r##"<path d="M {MAP_X:.2} {grid_y:.2} H {:.2}" stroke="#f6f3e7" stroke-opacity=".22" stroke-width="1" stroke-dasharray="7 8"/>
<text x="{:.2}" y="{:.2}" fill="#c8dae2" font-family="Segoe UI,Arial,sans-serif" font-size="17" text-anchor="end">{}</text>"##,
            MAP_X + MAP_W, MAP_X - 13.0, grid_y + 5.0, latitude_name(lat)
        );
    }
    let _ = writeln!(
        svg,
        r##"<rect x="{MAP_X}" y="{MAP_Y}" width="{MAP_W}" height="{map_h}" fill="none" stroke="#abbcc9" stroke-width="3"/>
<text x="{key_x}" y="235" fill="#d6e9e6" font-family="Segoe UI,Arial,sans-serif" font-size="32" font-weight="700">LEGEND</text>
<path d="M {key_x} 253 H 2270" stroke="#537184" stroke-width="2"/>
<text x="{key_x}" y="293" fill="#9fb6c3" font-family="Segoe UI,Arial,sans-serif" font-size="22">{units}</text>"##
    );

    let max_entries = 19usize;
    for (i, item) in legend.iter().take(max_entries).enumerate() {
        let y = 326.0 + i as f64 * 44.0;
        let color = hex(item.rgb);
        let name=escape_xml(&item.label);
        let _=writeln!(
            svg,
            r##"<rect x="{key_x}" y="{y:.2}" width="30" height="24" rx="3" fill="{color}" stroke="#a2b0bf" stroke-width=".7"/>
<text x="1865" y="{:.2}" fill="#e2e9e9" font-family="Segoe UI,Arial,sans-serif" font-size="20">{name}</text>"##,
            y + 20.0
        );
    }
    if legend.len()>max_entries {
        let _=writeln!(svg,r##"<text x="{key_x}" y="1187" fill="#b6c8cc" font-size="20" font-family="Segoe UI,Arial,sans-serif">+ {} more classes in metadata</text>"##, legend.len()-max_entries);
    }

    // A scale bar is geometrically meaningful only at the equator for this
    // equidistant-longitude display, not at every latitude.
    let equator_km = std::f64::consts::TAU * 6371.0;
    let bar_km = 5_000.0;
    let bar_w = MAP_W * bar_km / equator_km;
    let bar_y = map_bottom + 62.0;
    let _=writeln!(
        svg,
        r##"<path d="M {MAP_X:.2} {bar_y:.2} h {:.2}" stroke="#f0e2bf" stroke-width="12"/>
<path d="M {MAP_X:.2} {:.2} V {:.2} M {:.2} {:.2} V {:.2}" stroke="#f0e2bf" stroke-width="3"/>
<text x="{MAP_X:.2}" y="{:.2}" fill="#e5d4ae" font-size="20" font-family="Segoe UI,Arial,sans-serif">0</text>
<text x="{:.2}" y="{:.2}" text-anchor="end" fill="#e5d4ae" font-size="20" font-family="Segoe UI,Arial,sans-serif">5,000 km · at equator only</text>
<text x="{MAP_X:.2}" y="{:.2}" fill="#93abb8" font-size="20" font-family="Segoe UI,Arial,sans-serif">Fictional planet coordinates · southern latitudes at top · east at left</text>"##,
        bar_w, bar_y-12.0,bar_y+12.0,
        MAP_X+bar_w,bar_y-12.0,bar_y+12.0,
        bar_y+34.0, MAP_X+bar_w,bar_y+34.0,bar_y+75.0
    );

    let _=writeln!(
        svg,
        r##"<path d="M 120 1396 H 2280" stroke="#3f5a70" stroke-width="2"/>
<text x="120" y="1434" fill="#abc5ca" font-family="Segoe UI,Arial,sans-serif" font-size="22">Source: native planetary dataset · classified and styled programmatically · {} × {} raster</text>
<text x="120" y="1473" fill="#90a9b8" font-family="Segoe UI,Arial,sans-serif" font-size="18">Interpretations are fictional scientific scenario data; map styles do not establish a validated geological or climate simulation.</text>
<text x="2280" y="1474" text-anchor="end" fill="#8da5b5" font-family="Segoe UI,Arial,sans-serif" font-size="19">Aegaea Studio · SVG atlas sheet v1</text>
</svg>"##,
        image.size[0],image.size[1]
    );
    if let Some(parent)=path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path,svg).map_err(|e| format!("{}: {e}",path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn xml_escaping_covers_special_characters() {
        assert_eq!(escape_xml("A&B <world> \"x\" 'y'"),
                   "A&amp;B &lt;world&gt; &quot;x&quot; &apos;y&apos;");
    }

    #[test]
    fn exported_svg_has_embedded_map_legend_and_orientation() {
        let world=World::empty(PathBuf::from("not-aegaea"));
        let art=Cartography::new(&world);
        let path=std::env::temp_dir().join(format!("aegaea-layout-{}.svg",std::process::id()));
        export_sheet(&world,&art,Theme::Terrain,"My <New> & World",&path).unwrap();
        let svg=std::fs::read_to_string(&path).unwrap();
        assert!(svg.contains("data:image/png;base64,"));
        assert!(svg.contains("My &lt;New&gt; &amp; World"));
        assert!(svg.contains("southern latitudes at top"));
        assert!(svg.contains("Continental slope"));
        assert!(svg.ends_with("</svg>\n"));
        let _=std::fs::remove_file(path);
    }

    #[test]
    fn real_world_plate_legend_is_from_dataset() {
        let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/aegaea/tectonics/v1");
        let world=World::load(root).unwrap();
        let art=Cartography::new(&world);
        let legend=art.legend(&world,Theme::Plates);
        assert_eq!(legend.len(),17);
        assert!(legend.iter().any(|entry|entry.label=="MERROW"));
    }
}
