# Publication atlas sheets — SVG v1

**Purpose:** Produce printable, clearly labeled physical-world maps rather than bare colored rasters.

The composer embeds the native-resolution PNG map inside a self-contained SVG. It places scalable vector titles, graticule, color-coded thematic legend, an equator-only scale bar and the source/provenance notes around the image. Sheet export requires no GPU window, bundled fonts or external assets.

The existing synthetic atlas frame is mirrored/inverted: +180° longitude is left, -180° is right, -90° latitude is top, +90° is bottom. The sheet's labels preserve this unusual orientation. The reference radius used for the scale annotation is 6,371 km and will need to become per-planet metadata before scientific planetary generalization.

From the Studio toolbar: **Export atlas sheet SVG**. From the command line: \`cargo run -- --export-maps maps/aegaea\` generates an \`.svg\` plus the existing \`.png\` and \`.json\` for every available map. Its SVG contains a base64-embedded PNG; the map raster itself is not vectorized.

**Limits:** This is a fixed initial print layout, not an interactive page designer. The existing geological data are synthetic scenario data and the layout does not imply solved tectonic processes. The globe's geometry is a separate milestone.
