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

## Renderer visual defect gate (priority)

Actual user recording: 2026-11-10 22:11. Observed harsh polar pinwheel, very dark 3D globe, noisy fake isolines, broad flat-looking relief, and letterboxed 2D editor. We fix three direct rendering errors before expanding science tools: outward triangle winding + backface culling (no fake fragment-only hemisphere), consistent polar collapse, and single-source cartographic contrast. Separate the **data fidelity gate** from rendering correctness: Aegaea's synthetic mountain morphology requires a reproducible tectonic-to-topography refinement process. Do not claim a palette or GPU patch solves unrealistic terrain shapes.

Next after renderer visual acceptance: (a) area- and geology-aware DEM diagnostics and global-to-regional quality tiers, (b) opt-in multi-scale terrain synthesis constrained by source crust/plate/province fields; preserve reference elevations, (c) proper vector isolines, fault labels and calibrated projections, (d) retest both atlas and globe in a desktop capture.

## P1 terrain dependency inversion: actual generated elevation (v1)

The next main application now uses the non-elevation geological fields to calculate an independent DEM, with a documented source mode, saved seed, repeatable terrain output and parallel access to the old DEM as a comparison. Required: real Aegaea passes generation with the old elevation field deliberately absent, output is deterministic, scientific fields maintain native orientation, and all derived maps/export paths use the newly generated DEM. The process is currently a tectonic-class heuristic. The next steps are independent province geometry from plate motion and native-scale regional synthesis with explicit physical forcing; do not substitute palette cleanup for source model changes.

## Euler-derived boundary forcing checkpoint

A separate `plate_kinematics.rs` module now calculates present-day signed closure, spreading and tangential slip at plate-ID edges from the optional Euler poles/rates recorded in `plates.csv`. Derived terrain adds a small, explicit **heuristic** topographic response and exports independently inspectable kinematic raster layers. These fields are entirely independent of inherited DEM height values. This is **not** a history reconstruction, subducting-slab polarity model, or uplift-rate integration.

The scientific acceptance gate is a passing real-world Aegaea headless export and Windows unit tests, **followed by desktop visual review**. The next model step is to define subduction polarity and independently authored plate-stage topology before estimating large-scale uplift, drainage, erosion or climate. Avoid treating prettier maps as evidence that underlying geology is correct.

## Monthly scientific dataset foundation (implemented authoring; NOT climate simulation)

The independent project workspace now accepts typed, explicitly sourced monthly numerical grids. Monthly air temperature and precipitation are the first two maps using that registry. Users select a month, explicitly initialize its assumed input, paint directly in the atlas/globe, copy months only by deliberate action, and save sparse grids and their source metadata inside their own `world.json`. Compiler and rendering tests protect legacy geology, missing-month semantics, numeric range checks, provenance, dependency cycles and project round-trips.

The P4 physical model is **still unsolved**. Planned sequence: (1) planet-specific calendar/rotation/insolation state; (2) geophysical sources (land/ocean heat capacity, humidity and wind); (3) validated seasonal atmospheric and ocean heat transport; (4) physically bounded rain and evaporation; (5) derived annual and Köppen maps only with all prerequisite months, explicit calendar conventions and numerical acceptance gates. No hand-authored climate field must be silently labeled a solver result.

## Deterministic annual climate summaries (implemented numeric reduction; NOT a climate solver)

Annual air-temperature mean (°C) and annual precipitation total (mm/year) are two read-only calculated map styles based on all twelve explicitly authored monthly source slices. If any month is missing, the corresponding annual map remains unavailable. Temperatures now use calendar-solar-day-weighted means, with the weights read from the project-specific planet calendar; rainfall sums twelve monthly totals. Both derived fields carry an operator ID and source lineage, refreshing when source authoring or the calendar changes. No rainfall or temperature physics is synthesized.

Next scientific gate: planet-specific calendar duration, solar geometry, pressure, moisture/wind and surface flux inputs; only then should the program attempt a coupled atmosphere/ocean moisture-and-energy simulation. Derived annual maps must retain numeric and provenance consistency under re-open, edit, and export.

## Planet-specific calendars and seasonal TOA solar geometry (implemented)

The project-scoped calendar stores twelve month durations (local solar days), mean solar-day hours, obliquity, year-start solar longitude and assumed incident stellar flux. Inputs are validated before application, saved in `world.json`, and default in older projects without modifying source data. Orbit duration follows the sum of twelve month lengths; the present model assumes a **circular, zero-eccentricity** orbit and no precession. Month means integrate daily-mean TOA irradiance over each source period (W/m²) and are displayed as the nineteenth thematic map in both projections. The derived provenance includes the calendar and operator, and climate datasets remain user-authored, not implied by the solar field.

**Next science gates:** orbital eccentricity / anomaly and astronomical epoch conventions; stellar and orbital dynamics; atmospheric pressure, heat capacities, land/ocean surface energy exchange, and humidity/wind sources. Do not label TOA insolation a predictor of precipitation, temperature, clouds, seasons of life, or Köppen classes.

## P4 atmospheric boundary-condition inputs (implemented manual sources; NO flow physics)

Four optional monthly source fields are now authorable and inspectable in 2D/3D: surface pressure (hPa), specific humidity (g/kg moist air), and geographic east/north wind components (m/s). Their values are explicitly authored, separated by time period, range/unit validated, undoable and persisted as sparse project-local datasets. Signed wind components preserve physical direction; the inverted/mirrored presentation projection does not invert source coordinates. Each map exports metadata with true units and author provenance. An additional static source map carries surface areal heat capacity (MJ/(m²·K)) for user-painted land/ocean thermal assumptions, without automatically classifying terrain or choosing ocean mixed-layer depth.

**Next numerical gate:** energy/moisture flux dimensional consistency, mass-conserving transports, stable advection on the spherical grid, land/ocean boundary conditions, and diagnostic error budgets. Do not treat default source values as computed atmospheric circulation or rainfall. Equilibrium pressure, precipitation and winds cannot be deduced from these hand-authored inputs alone.

## P4 numerical moisture-transport checkpoint (implemented conservative preview, NOT climate coupling)

A first-order spherical finite-volume operator now converts author-specified surface pressure and specific humidity to a **vertically uniform column-vapor proxy** via `W = (100 * p_hPa / g) * (q_g_per_kg / 1000)` kg/m², then advects W under the two authored horizontal wind components. Geographic positive-east wind travels toward decreasing atlas x indices; longitude is periodic, north/south boundaries are closed. Flow faces exchange equal and opposite kg; spherical cell areas weight diagnostics. A maximum 0.75 outward Courant bound determines stable explicit substeps and an upper work budget rejects oversized intervals. Tests cover sign/direction, pole and seam integrity, global 4πR² area consistency, mass conservation including float32 map export, positivity, missing inputs, source preservation, and month invalidation.

The resulting read-only 25th map shows transported water mass (kg/m²); the conservation budget is inspectable and machine-readable. There is **no** pressure evolution or continuity for dry air, vertical stratification, water source/sink, condensation, evaporation, or dynamical wind solver. Thus passing a numerical conservation test cannot justify rain or climate forecasts. Future acceptance requires spherical mass-consistent airflow, moisture thermodynamics/condensation, land-ocean energy balance and bounded multi-month cycles.
