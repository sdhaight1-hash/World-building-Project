# Raster graph v1 — contract and examples

The first processing graph supports five explicit node operators:

| Operator | Input | Output | Meaning |
| --- | --- | --- | --- |
| \`elevation\` | World elevation raster | elevation metres | Import current native DEM, after authored project overlay |
| \`elevation_offset\` | elevation metres | elevation metres | Arithmetic height adjustment (not physical uplift/tectonics) |
| \`slope\` | elevation metres | slope degrees | Great-circle-metric-aware gradient on a spherical raster |
| \`orographic_exposure\` | elevation metres | signed index | Existing terrain-only directional rain-shadow proxy |
| \`output\` | any available field | same quantity | Publish a named numeric map |

Recipe schema \`version:1\` is JSON. Each node has a unique safe ID and an \`operator\` tag. Input edges are other node IDs. The \`outputs\` list contains the IDs of explicit Output nodes. Inputs must satisfy unit/type constraints.

Run against Aegaea:

\`\`\`powershell
cargo run --release -- --build-graph docs/examples/aegaea-map-graph.json maps/generated
\`\`\`

With a custom source project:

\`\`\`powershell
cargo run --release -- --project C:\MyPlanet --build-graph C:\MyPlanet\recipe.json C:\MyPlanet\generated
\`\`\`

Outputs are native-grid Float32 TIFF files plus JSON sidecars containing node graph recipe, quantity/units, dimensions and coordinate convention. These TIFFs are **not georeferenced EPSG:4326**. There are no data-baking side effects on the imported source.

### Validation and limitations

- Reject cycles, missing inputs, duplicate IDs, invalid filenames, incompatible numeric units and non-finite offset values.
- Reuse shared dependencies within a single evaluation; never recompute a branch twice.
- Slope requires positive planet radius; uses physical spherical pixel spacing and guards polar singularities approximately.
- Source grid and manual modifications remain untouched; run the same recipe twice for deterministic outputs.
- No persistent content-addressed cache, GPU processing, tiling, advanced visual graph, source hash validation or automatic dirty-region invalidation yet.
- Not every analytical node is a physical simulation; elevate only well-validated models to that category.

Do not claim that a 0-metre offset reconstructs geology, that a terrain-only exposure index predicts rainfall or that a map graph itself models actual tectonic history.
