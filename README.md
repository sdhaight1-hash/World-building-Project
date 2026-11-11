# Aegaea Studio — physical-world cartography

A **Rust + wgpu** desktop application and an included, independent, data-rich fictional world: **Aegaea**.

**Mission:** create, edit, analyze, visualize, and publish physical-world maps and their underlying data. The project focuses on planetary geography, geology, oceans, climate, hydrology, soils, ecology, natural resources and natural hazards. **No voxel engine and no human geography/civilization simulation.**

## Two products developed together

1. **General-purpose Studio:** reusable editing, mapping and data tools for *any* fictional planet. No hardcoded Aegaea plate IDs, craton names, geography, or units.
2. **Aegaea reference project:** a working, inspectable example built with the same tools. The imported geological dataset is provisional; detailed tectonic reconstruction has **not** yet been simulated.

The existing geological source files remain under [`data/aegaea/tectonics/v1/`](data/aegaea/tectonics/v1/), with instructions for restoring large split assets.

## Current implementation

The `studio/` directory contains an **initial desktop vertical slice**, not a complete mapping suite. It provides:

- A dark, map-first Rust/egui desktop interface backed by **wgpu**.
- A 2D atlas and a **wgpu-rendered height-displaced 3D globe mesh** using the same underlying native geological fields, terrain DEM, and thematic styles. No voxel engine.
- **Twenty-nine map styles** including physical relief/bathymetry, tectonic plates, classified plate contacts, craton IDs, craton basement age, named crustal provinces, crust thickness, lithosphere thickness, heat flow and rain-shadow exposure.
- Built-in legends, native-resolution **PNG + JSON map metadata**, and self-contained **publication SVG sheets** with vector titles, legends, graticules, equatorial scale annotations, and provenance.
- Thematic display of terrain, plate IDs, cratons, crustal provinces and selected numeric geophysical fields.
- **Dedicated Rain Shadow map**: DEM-derived windward/leeward exposure, selectable incoming wind, inspector, legend, TIFF + metadata export. This is not a precipitation simulation.
- Geographic coordinate picking and a data inspector; plate and craton names come from CSV records.
- User-authored geographic point annotations stored separately from the read-only source dataset.
- **Non-destructive terrain brush:** sculpt or raise/lower a DEM directly in either 2D or 3D, undo/redo strokes, persist sparse project-local height overrides, and export edited numeric TIFF data without changing the original DEM.
- **New blank project in the desktop UI:** click **New project…**, enter a project name and unused folder, and create an independent 1024×512 editable flat-ocean world without restarting Studio. The new `world.json` is saved immediately; Aegaea geological layers, markers, brush history, and map selection are not carried across. Existing folders are never overwritten, and switching away from unsaved work requires explicit confirmation.
- **Blank world terrain initialization:** a new blank project starts with a -2,000 m editable flat-ocean DEM; sculpt, add locations, and **Save** to persist your edits.
- **Project-local map layer authoring:** blank projects can create all six current source-data families—plate IDs, craton IDs, crustal province classes, crust thickness, lithosphere depth and surface heat flow—and paint them in **both** the 2D atlas and the 3D globe. Authors can add plates and cratons, name them, set craton ages and plate Euler rotations, and undo/redo layer strokes. Layer assumptions and sparse edits persist in `world.json` without changing imported TIFF files. Existing derived maps (plate boundaries, motion, slope, rain shadow and basement age) become available when their scientific inputs exist.
- **Monthly climate data and maps:** create separate air-temperature (°C) and precipitation (mm/month) datasets for each calendar month; select the month, author numeric values directly in 2D or 3D, copy an existing month only by explicit action, and inspect/export the active month's map. Scientific field records include units, limits, declared provenance, time slices and dependency validation. Sparse changes persist inside `world.json`, not 12 full-resolution rasters per field. **These are authored climate inputs, not a climate or rainfall simulation.**
- **Planet calendar and solar geometry:** each project now stores an editable 12-period calendar (individual month lengths), mean solar-day duration, obliquity, solar longitude at year start and incident stellar flux. The **Monthly top-of-atmosphere sunlight** map is computed from a circular-orbit, latitude-aware astronomical model in the atlas and 3D globe (W/m²), with source parameters and month recorded in export metadata. All defaults are author assumptions, not generated climate.
- **Atmospheric input maps:** independently author monthly surface pressure (hPa), water-vapor **specific humidity** (g/kg moist air), and signed eastward/northward wind components (m/s). Every month is an optional, separate sparse source with validation, undo/redo, and month-specific atlas/globe exports. These are author-defined source fields, not calculated pressure or atmospheric circulation.
- **Surface thermal storage:** create and paint a stationary **surface areal heat capacity** layer (MJ/(m²·K)) in Map Layer Workshop. The illustrative **Land-like · 2** and **Ocean mixed layer · 200** presets select only the paint value: you decide where to paint them. No land/ocean mask, mixed-layer depth, or thermal physics is inferred.
- The independent command-line blank-world path remains available.
- Core mathematical/data tests and a Windows GitHub Actions workflow.

**Not yet implemented:** GPU-native terrain geometry, vector editing, layer compositing, geological reconstruction through time, climate/hydrology generation, advanced map layouts, or multi-format export. The initial globe is an orthographic **software-projected map texture uploaded to wgpu**; this is deliberately a bootstrap path, not the final 3D terrain renderer.

## Run on Windows

Install a stable Rust toolchain (rustup) and run from the repository root:

```powershell
cargo run --release -p aegaea-studio
```

On first run the application loads the committed reference rasters in `data/aegaea/tectonics/v1`, including the elevation TIFF directly from its split binary parts when necessary. It stores personal annotations at `projects/aegaea-workspace.json` (ignored by git). The immutable geological data remains untouched.

To create a separate, editable world from the desktop app, click **New project…** in the top toolbar. Enter a name and a **new, unused** project directory (for example `projects/my-world`), then click **Create blank project**. The world is saved to `projects/my-world/world.json` immediately. Use **Save** after sculpting or adding markers.

To author a new geological or numeric map, select its style under **Map library** (even if marked **needs source**), then use **Map Layer Workshop → Create source layer**. Choose **Paint source layer** and a category or numeric value; drag in either map view and Save. Numeric initial fills are **author assumptions**, not generated or scientifically validated results. To build plate-motion maps, add at least two plates, paint their IDs, and define Euler pole/rate metadata for every plate. To build basement-age maps, create craton records and paint their IDs. For user-classified tectonic contacts, select **Plate boundary classes**, choose a contact type, enable **Draw classified contact (two clicks)** and click both endpoints on the atlas or globe; the result is stored in the project and may be removed without modifying source files. Geological terrain synthesis additionally needs both oceanic and continental province classes plus plate and crust layers; it does not infer those categories automatically.

**Supported now:** 29 thematic map styles, including calendar-driven monthly sunlight, authored temperature and precipitation, annual duration-weighted temperature, and annual total rainfall. **Not yet supported:** elliptical orbital mechanics, coupled climate/atmosphere/ocean simulation, hydrology, soils, ecology, resources and hazards. Authored values are assumptions; astronomical sunlight is top-of-atmosphere energy, not ground temperature or rainfall. No Köppen classification is implied.

For atmospheric inputs, choose **Monthly surface pressure**, **Monthly specific humidity**, **Monthly wind · eastward component**, or **Monthly wind · northward component** under Map Library. Select the active month, explicitly create its source, and paint from the atlas or globe. Positive wind values mean **eastward** or **northward** in geographic coordinates, regardless of the atlas's mirrored orientation; negative values mean westward/southward. Dynamical winds can now be explored in a separate **read-only, linearized pressure-gradient + Coriolis test** alongside the fixed-wind air/vapor transport preview; neither is a complete circulation or rainfall model. For thermal storage, choose **Surface areal heat capacity** and create a source layer, then paint your author-specified values.

For climate authoring, select **Monthly air temperature** or **Monthly precipitation** under Map library. Choose a month in **Monthly Scientific Data**, enter an initial assumed value, and choose **Create this month's source**. Enable **Paint monthly values** to edit it on the map or globe. Switch months to create distinct records (or explicitly copy an existing month); missing months are never automatically filled. Save and reopen your world with `--project projects/my-world` to recover the chosen month and all authored values. PNG/SVG exports use month-specific filenames; PNG sidecar JSON carries `dataset_id`, `month`, `units`, and `provenance`.

Once **all twelve months** of a quantity have been created, select **Annual air temperature · day-weighted mean** or **Annual precipitation total** in Map library. Annual temperature weights monthly mean temperatures by the solar-day durations in the configured planetary calendar; rainfall sums the twelve monthly totals. Incomplete years do not produce an annual field. The annual map automatically updates after completing a climate paint stroke or switching month, is inspectable, and exports with a year-specific map name and a derived-provenance record.

To configure a planet, expand **Planet calendar & orbit** in the sidebar. Change the twelve month lengths (each in local solar days), solar-day hours, axial tilt, year-start solar longitude (0° is northern spring equinox), or incident stellar flux (W/m²), then click **Apply calendar & solar settings**. Choose **Monthly top-of-atmosphere sunlight**, then select any month to see its computed mean daily sunlight on the atlas or globe. The default is a **fictional 360-day, 24-hour solar-day, circular-orbit** scenario with 23.44° tilt and 1361 W/m² at orbital distance. The stellar/orbital parameters are assumptions. Day and month edits do not synthesize temperature, rainfall, clouds or circulation. Saved worlds using the old project format acquire these defaults non-destructively on load.


### Coupled air and vapor continuity experiment

Choose **Transported vapor-column proxy**, **Pressure after transport · diagnostic**, or **Humidity after transport · diagnostic** from the map library. For the selected month, create authored **surface pressure**, **specific humidity**, and **eastward/northward wind** sources. In the transport panel set the experiment duration (default 120 seconds), and optionally adjust the project's **radius** and **surface gravity** in Planetary physical constants (Earth-like defaults: 6,371 km and 9.80665 m/s²). Click **Compute read-only coupled preview**. The resulting three map styles have units **kg/m²**, **hPa**, and **g/kg**. Each has a numeric inspector and native map export.

The operator represents moist-air column mass as `M = (100 × p_hPa) / g` kg/m² and the water fraction as `q = q_g/kg / 1000`. It applies **the same spherical finite-volume upwind face fluxes to air mass and water mass**, using true spherical cell areas, a periodic longitude seam, closed polar boundaries, and the Aegaea longitude convention. Pressure and specific humidity are then **diagnosed from the transported masses**. Air, dry-air and water conservation follow from common upwind fluxes; both air and vapor residuals are evaluated on exported float32 outputs with 1e-6 relative tolerance. A 0.75 outgoing Courant bound sets substeps; overly long runs are rejected. The inspector reports before/after air and vapor mass, errors, pressures, humidity bounds, and substeps. Diagnostics export to JSON alongside map PNG/SVG.

**Read-only:** This is a numerical preview, not an authored source. It never changes monthly pressure, humidity, wind, temperature or precipitation. Source edits, changing months, changing physical constants, or reopening a world clears the preview. Changing planetary constants does not silently regenerate terrain.

**Scientific limits:** Authored winds remain fixed: no prognostic wind/momentum dynamics, Coriolis or pressure-gradient acceleration, terrain-atmosphere coupling, vertical stratification, evaporation, condensation, cloud formation, precipitation, radiation or climate predictions. Water vapor is represented by the intentionally simplified *vertically uniform humidity* proxy. Conserving air/water numerically does not demonstrate a realistic weather model.

### Experimental rotating pressure-gradient wind preview

Select **Wind after dynamics · eastward** or **Wind after dynamics · northward** (or an existing transport diagnostic) under Map library. With authored monthly pressure, humidity and both winds present, use **Compute evolving-wind preview**. This is separate from **Compute read-only coupled preview**, which keeps winds fixed. The new mode adds two read-only maps (**29 total**), and also updates the three existing transport outputs. The editor reports maximum wind before/after, maximum pressure-gradient acceleration, inferred rotation period, effective wave speed, both global mass residuals, and outgoing/gravity-wave Courant numbers. All five maps export with the numerical method and assumptions in PNG/JSON metadata.

The wind tendency uses `du/dt = -c² ∂ln(p)/∂east + fv`, `dv/dt = -c² ∂ln(p)/∂north - fu`, with `f=2Ω sin(latitude)`; Coriolis turning is integrated as an exact local rotation. An **author-specified, persisted effective barotropic wave speed** c defaults to 80 m/s; it is *not* an inferred air temperature, acoustic sound speed, or physically calibrated vertical mode. For an assumed **prograde circular orbit**, rotation is inferred from the planet's authored **mean solar day** and **orbital year**: `Ω=2π/Tsolar+2π/Torbit` (times in seconds). Solar day must not be mistaken for sidereal spin. Retrograde rotation is not supported in this prototype.

The wind/pressure tendencies update in short stable steps and feed the **same** coupled finite-volume flux used for air/vapor mass conservation. Stability is bounded by a gravity-wave and advective timestep, with stricter resolution requirements near spherical polar rows; oversized intervals are refused. All derived maps are ephemeral and invalidate on source, calendar, radius, gravity, or wave-speed changes. A default dynamic interval of **10 seconds** keeps high-resolution maps responsive without pretending to integrate climatological time scales.

**This is an experimental linearized momentum tendency, not complete spherical shallow-water physics or real weather.** It omits nonlinear momentum advection, spherical curvature/metric terms, vertical structure, friction, energy closure, pressure-temperature coupling, evaporation, condensation, and precipitation. Polar convergence and global angular momentum require separate validation. Numerical mass conservation alone does not establish model realism. The full spherical shallow-water model requires stronger benchmarks, such as [Williamson et al.'s standard test suite](https://www.sciencedirect.com/science/article/pii/S0021999105800166).

### Spherical barotropic momentum and global budgets

A third, opt-in action under the atmospheric map workshop — **Compute conservative spherical momentum preview** — advances the same five ephemeral output maps (pressure, specific humidity, vapor column, eastward wind, northward wind). It uses the existing **dynamic interval**, **effective barotropic wave speed**, planet radius, surface gravity and solar-day/orbital-year rotation assumptions. Neither authored atmospheric fields nor terrain are modified.

Unlike the simpler linearized-wind action, this method transports **absolute axial angular momentum** `a cosφ [u + Ω a cosφ]` as a mass-weighted quantity with the *same face transfers* as moist air and vapor. The pressure torques on the two sides of each longitude face are equal and opposite. The northward momentum balance includes nonlinear spherical curvature and Coriolis terms, plus geometric balancing for a uniform resting pressure field. Physical meridian-integrated face moment arms, latitude-band areas and zero-length polar boundaries avoid pretending pole cells are rectangles. A polar CFL check limits stable steps and refuses oversized workloads rather than inventing wind motion at a numerical singularity.

The diagnostic panel and exported JSON report global **air, vapor, and axial angular momentum** before/after with strict published-raster residuals (each ≤1e-6). It also reports kinetic and relative barotropic compressional energy and **energy drift**. This is a measured diagnostic, *not* an assertion that the numerical method conserves energy. The first-order upwind scheme is dissipative.

**Scientific limit:** This is a vertically integrated *isothermal barotropic column* with assumed pressure closure `P = c² × column mass density`, not the full three-dimensional atmosphere or a calibrated free-surface shallow-water solver. Northward component transport, polar topology, full energy closure, longitudinal angular-momentum distribution and climate realism still require stronger numerical tests. In particular, [NOAA/GFDL's discussion of axial angular momentum](https://www.gfdl.noaa.gov/wp-content/uploads/files/model_development/ocean/mom3_manual.pdf) distinguishes the globally conserved axial quantity from local zonal and meridional momentum. Passing the conservation tests here **does not** establish valid rainfall, weather, or long-period circulation.

The command-line blank-world bootstrap is also supported (its project file is written when you save in the app):

```powershell
cargo run --release -p aegaea-studio -- --new projects/my-world
```

To open an existing project directory containing compatible TIFF layers:

```powershell
cargo run --release -p aegaea-studio -- --project C:\path\to\my-world
```

Build the reference world's atlas maps without opening a GPU window:

```powershell
cargo run --release -- --export-maps maps/aegaea
```

Each available map is saved at the source grid resolution with a JSON legend. The exported PNG is a **data-styled map image**, not a GeoTIFF and not yet a publication-layout page with embedded legend and labels. See [data-driven cartography](docs/CARTOGRAPHY.md).

Run the tests:

```powershell
cargo test --workspace
```

See [Product definition](docs/PRODUCT.md), [architecture](docs/ARCHITECTURE.md), [map catalog](docs/MAP_CATALOG.md), [UX standards](docs/UX.md) and [roadmap](docs/ROADMAP.md).

## Data integrity and provenance

Aegaea's current dataset uses a **mirrored/inverted fictional atlas convention**, not ordinary Earth EPSG:4326: left is +180° longitude, right is -180°, top is -90° latitude, bottom is +90°. The native scientific grid is **1672 × 941**; 8K atlas images are presentation upscales. The viewer keeps this convention on **both** map projections.

Generated, imported, hypothetical and manually authored information must remain distinguishable. A raster-based present-day tectonic model is not equivalent to a solved deep-time geological reconstruction.

## Working agreement

Changes to editors and models should be exercised on two fixtures: (1) Aegaea (complex imported data) and (2) a blank independent world. Every implemented generator should eventually deliver a reusable tool **and** a corresponding documented Aegaea map. Keep code narrow, verify changes, and never silently overwrite authored work.

### Terrain authoring workflow

Choose **Terrain brush** in the left sidebar, then **Raise** or **Lower**, adjust radius and strength, and drag on either map view. Each drag is one undoable stroke, with Ctrl+Z / Ctrl+Y shortcuts. Changes are saved to your project JSON and applied over the immutable elevation raster on load. **Export authored DEM TIFF** writes a numeric floating-point raster plus metadata; the original source is not modified.

The artist's brush is not a validated geophysical terrain generator. Contours, relief, the 3D mesh, and the terrain-only rain-shadow index are recalculated when a stroke completes. On large worlds this may briefly interrupt interaction; incremental recomputation is a later optimization. See [terrain authoring specification](docs/TERRAIN_EDITOR.md).

## Procedural map graph — first functional processing pipeline

The Studio now has a **typed deterministic node graph runtime** for numeric map data. The initial v1 operators are **Elevation Input**, **Elevation Offset**, **Slope (degrees)**, **Orographic Exposure**, and **Output**. The graph rejects missing inputs, circular dependencies, mismatched scientific units, duplicate node IDs, and invalid output names. Multiple outputs can share computation, and the original imported DEM remains unmodified.

Generate two actual Aegaea data maps from the example recipe:

```powershell
cargo run --release -- --build-graph docs/examples/aegaea-map-graph.json maps/generated
```

The output folder contains **slope_degrees.tif** and **rain_shadow_potential.tif**, plus recipe/provenance JSON for each. This is an initial **headless processing graph**; an interactive visual node editor, tiled processing, cache invalidation across builds, scientifically calibrated wind fields, and erosion solvers are future tasks. See [the architecture decision](docs/WORKFLOW_INSPIRATIONS.md) and [node-graph contract](docs/MAP_GRAPH.md).

### Surface slope map

Choose **Surface slope** in the map library to visualize true surface slope angle in degrees. The output is computed from the currently authored elevation grid through the same spherical slope calculation supported by the procedural map graph, and is recalculated when terrain changes. It is available in both 2D and 3D, the location inspector, native PNG export and SVG atlas sheets.

### Geology input diagnostics

Select **Geology consistency** → **Validate source layers** in the map sidebar, or run `cargo run -- --diagnose-world maps/diagnostics/geology-consistency.json`. The JSON report checks that source raster category IDs exist in source tables and measures where craton pixels disagree with their assigned parent plates. Results are warnings/diagnostics only; the application does not secretly alter tectonics to match a painted heightmap. Reports count grid cells, not physical square kilometres.

## Rendering quality checkpoint — gameplay/video visual QA

The 2026-11-10 desktop recording exposed globe polar pinwheel geometry, dark double-shaded maps, aliased fake contours, a heavy single-pixel coastline and unused atlas work area. The first correctness pass (see `docs/RENDER_QUALITY.md`) addresses those specific issues **without replacing the geological source DEM**.

Choose **Fill workspace** for an aspect-preserving cropped viewport with normal drag/scroll controls, or **Fit entire planet** for the original whole-map perspective. On the globe, triangles now have outward winding and the far hemisphere is GPU-culled; poles converge to a single elevation, and the globe-only color texture is harmonized near the geographic pole. The globe's second lighting pass is much lighter, keeping the thematic legend readable.

**Not solved by this fix:** the source terrain still contains obvious procedural oval mountain belts, coarse landform morphology, and unrefined drainage/coastlines. Those need generation-model improvements and source-preserving derived terrain, not new palettes.

## Geological terrain generator (v1)

Aegaea no longer needs to inherit the original elevation map as the basis for its physical maps. **Untouched geological projects default to a newly generated elevation field**, constrained by the scenario's province classes, crust thickness, cratons and plate boundaries. The source `elevation_m.tif` stays available for side-by-side comparison and old authored projects.

Open **Terrain Source / Generator** to change seed and relief strength, regenerate, or return to the legacy DEM. Existing project-local brush edits cannot be silently retargeted to a different terrain basis. 2D/3D views, slope maps and rain-shadow maps use whichever numeric elevation field is selected. To export independent generation results:

```powershell
cargo run --release -- --generate-terrain maps/geology-generated
```

See [generation contract, provenance and limitations](docs/TERRAIN_GENERATION.md). **The first generator is heuristic, not a validated tectonic + erosion simulator.**

### Euler-based plate motion forcing

The source `plates.csv` already provides Euler poles and signed rotation rates. The terrain pipeline now computes present-day relative velocity at real plate contacts and classifies normal convergence, normal opening and tangential slip, exporting all three fields as numeric TIFFs alongside the separately identified *hypothetical* resulting relief. The calculations work without the legacy DEM; they are **instantaneous kinematics**, not simulated deep-time evolution, slab polarity or erosion. See [terrain generation](docs/TERRAIN_GENERATION.md).

### Plate-motion diagnostic maps

Select **Plate convergence**, **Plate spreading**, or **Plate shear** in the map library. All three are computed from Euler poles and real adjacent plate IDs (not the old elevation source), displayed on the native atlas and wgpu globe with separate cm/year color legends and per-location inspector readouts. The same numeric inputs are included in automated PNG/SVG map exports. Zero means no modeled motion *component at that sampled cell*, not verified tectonic inactivity. These are present-day velocity diagnostics, **not deep-time reconstructions or physically integrated elevation change**.
