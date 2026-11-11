# Development and reference-world roadmap

**Rules:** User decisions determine scope; small, verifiable milestones. Every major editor is tested both with Aegaea and a blank project. No silent data modification. We do not work on civilization maps or a voxel engine.

| Stage | Generic studio deliverable | Aegaea companion deliverable | Exit gate |
| --- | --- | --- | --- |
| P0 | Buildable Rust/wgpu shell; map catalog, source reader, 2D + globe, inspector, local annotation | Current geology layers inspectable in two synchronized projections | Build/test on Windows and manually verify view/coordinate parity |
| P1 | Editable map system; point/line/polygon geometry, spatial indices, style + legend, saved map layouts, undo | Refine plate and craton geometry and create polished tectonic atlas | Cross-view edit and save round trip; export with legend |
| P2 | Raster drawing, DEM transforms, bathymetry and subsurface sections | New physical relief, shelves, basins, hillshade and rock/age maps | Source-derived elevation is reproducible and override-safe |
| P3 | Time-aware geological editor (plate rotations, topology change, ages) | Testable continental history, sutures, rifts and changing ocean floor | Geological invariants validated; scenario uncertainty exposed |
| P4 | Atmospheric/ocean tool kit; time/month fields; climate fields | Seasonal wind, current, pressure, temperature, rainfall and climate maps | Unit consistency, area weighting, visual plausibility |
| P0.1 | Dedicated rain-shadow exposure thematic map; selectable wind direction; TIFF/metadata export | Aegaea DEM-driven windward/leeward exposure map | Synthetic ridge reverses with wind; ocean nodata; cross-view inspector and reproducible export |
| P5 | Hydrology/soils/biomes | Drainage and lakes; soils; natural ecosystems | Watershed continuity, seasonal comparison, explicit provenance |
| P6 | Mineral/resource/hazard models; print and export suites | Geology/resource/hazard atlases; complete documented reference world | Full reproducible project, portable data, print-quality cartography |

## Initial P0 boundaries

In P0, a **software-projected** orthographic globe texture (displayed using wgpu) is an intentional bootstrap. A true GPU mesh-based, height-displaced sphere is deferred; don't claim it is already delivered. Annotation support is point-only. Layer display is thematic selection, not full compositing.

### Engineering acceptance checklist

- `cargo fmt --check`, `cargo test`, `cargo check` on Windows CI
- Aegaea source grid dimensions read and validated; Aegaea project works when optional elevation has been restored
- Both views use same geo->raster formula and correctly handle mirrored/inverted orientation
- Distinct 2D/3D view state preserved between switches; no duplicated world arrays
- Markers/notes persist separately, with no mutation of source GeoTIFFs
- Blank independent project opens and saves without default Aegaea plate names or datasets
- No silently substituted or invented scientific values for missing layers

## P0.3 — Publication sheets

Reusable `publication.rs` exports self-contained SVG atlas pages from each physical-world map. Vector labels, thematic legends, a coordinate graticule and an equator-only scale bar are composed with the same native raster. No embedded font assets or GPU window required. This is the initial print layout foundation, not an editable print-layout engine.

## GPU globe milestone P0.4

Use `egui-wgpu` paint callbacks to render a DEM-displaced spherical triangle mesh in WGSL. Share the atlas layer texture, and preserve source coordinate orientation, selection and camera parameters. Acceptance: exact mesh counts; validation of the shader with Naga in headless tests; compilation on Windows; subsequent desktop visual verification on GTX 2060. Further LOD and custom planet radius metadata remain future phases.

## P1/P2 checkpoint — Non-destructive DEM authoring

An elevation painting tool lets the author raise/lower terrain directly in the 2D map and globe. Sparse absolute-elevation overrides are stored in the project sidecar, and grouped brush strokes are undoable/redoable without writing to the source TIFF. Completion recomputes hillshade, rain-shadow and the displaced globe mesh; numeric DEM/metadata export is provided. Blank worlds can be created with a flat-ocean seed elevation. Polygon/line editing and geological time reconstructions are still future work.

## New architecture checkpoint — physical-world data graph

The first `map_graph.rs` executor supports type-checked scalar raster nodes: imported/edited elevation, height offset, spherical slope and the existing rain-shadow exposure proxy. JSON recipes can be built headlessly into separate numeric TIFF fields with self-describing provenance. **Acceptance:** cycle/missing-input detection, unit checking, deterministic repeated output, shared dependency reuse, exact numeric export, and a successful reference-world build of `docs/examples/aegaea-map-graph.json`.

Next: offer those recipes as **quick tools in the editor**, then a dockable advanced node graph with instant preview and user-created links. Introduce tile-cache/dirty-region processing *before* computationally intensive erosion or planet-wide climate solvers. Only after conservation/validation gates should terrain hydrology and atmospheric nodes claim physical simulations.

## Geology consistency (implemented first input validation)

A non-destructive `geology_diagnostics.rs` pass checks that each categorical plate/craton/province raster value matches an actual source metadata row and that non-background craton pixels belong to the parent plate named by the corresponding craton record. It also flags missing parent plate names. Reports contain **cell counts**, explicitly not area-weighted geological statistics. Run with `--diagnose-world maps/diagnostics/geology-consistency.json` or press **Validate source layers** in the desktop sidebar. Nothing is silently corrected or conflated with physical simulation.
