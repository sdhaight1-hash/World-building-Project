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
- **Ten map styles** including physical relief/bathymetry, tectonic plates, classified plate contacts, craton IDs, craton basement age, named crustal provinces, crust thickness, lithosphere thickness, heat flow and rain-shadow exposure.
- Built-in legends, native-resolution **PNG + JSON map metadata**, and self-contained **publication SVG sheets** with vector titles, legends, graticules, equatorial scale annotations, and provenance.
- Thematic display of terrain, plate IDs, cratons, crustal provinces and selected numeric geophysical fields.
- **Dedicated Rain Shadow map**: DEM-derived windward/leeward exposure, selectable incoming wind, inspector, legend, TIFF + metadata export. This is not a precipitation simulation.
- Geographic coordinate picking and a data inspector; plate and craton names come from CSV records.
- User-authored geographic point annotations stored separately from the read-only source dataset.
- **Non-destructive terrain brush:** sculpt or raise/lower a DEM directly in either 2D or 3D, undo/redo strokes, persist sparse project-local height overrides, and export edited numeric TIFF data without changing the original DEM.
- **Blank world terrain initialization:** create a 1024×512 editable flat-ocean DEM in a new world, then sculpt and save it.
- An empty-project path to verify that the application is not coupled to Aegaea.
- Core mathematical/data tests and a Windows GitHub Actions workflow.

**Not yet implemented:** GPU-native terrain geometry, vector editing, layer compositing, geological reconstruction through time, climate/hydrology generation, advanced map layouts, or multi-format export. The initial globe is an orthographic **software-projected map texture uploaded to wgpu**; this is deliberately a bootstrap path, not the final 3D terrain renderer.

## Run on Windows

Install a stable Rust toolchain (rustup) and run from the repository root:

```powershell
cargo run --release -p aegaea-studio
```

On first run the application loads the committed reference rasters in `data/aegaea/tectonics/v1`, including the elevation TIFF directly from its split binary parts when necessary. It stores personal annotations at `projects/aegaea-workspace.json` (ignored by git). The immutable geological data remains untouched.

To create a separate, empty world:

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
