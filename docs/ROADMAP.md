# Development and reference-world roadmap

**Rules:** User decisions determine scope; small, verifiable milestones. Every major editor is tested both with Aegaea and a blank project. No silent data modification. We do not work on civilization maps or a voxel engine.

| Stage | Generic studio deliverable | Aegaea companion deliverable | Exit gate |
| --- | --- | --- | --- |
| P0 | Buildable Rust/wgpu shell; map catalog, source reader, 2D + globe, inspector, local annotation | Current geology layers inspectable in two synchronized projections | Build/test on Windows and manually verify view/coordinate parity |
| P1 | Editable map system; point/line/polygon geometry, spatial indices, style + legend, saved map layouts, undo | Refine plate and craton geometry and create polished tectonic atlas | Cross-view edit and save round trip; export with legend |
| P2 | Raster drawing, DEM transforms, bathymetry and subsurface sections | New physical relief, shelves, basins, hillshade and rock/age maps | Source-derived elevation is reproducible and override-safe |
| P3 | Time-aware geological editor (plate rotations, topology change, ages) | Testable continental history, sutures, rifts and changing ocean floor | Geological invariants validated; scenario uncertainty exposed |
| P4 | Atmospheric/ocean tool kit; time/month fields; climate fields | Seasonal wind, current, pressure, temperature, rainfall and climate maps | Unit consistency, area weighting, visual plausibility |
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
