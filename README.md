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
- A 2D atlas and a 3D globe **rendered from the same geological raster values**.
- Thematic display of terrain, plate IDs, cratons, crustal provinces and selected numeric geophysical fields.
- **Dedicated Rain Shadow map**: DEM-derived windward/leeward exposure, selectable incoming wind, inspector, legend, TIFF + metadata export. This is not a precipitation simulation.
- Geographic coordinate picking and a data inspector; plate and craton names come from CSV records.
- User-authored geographic point annotations stored separately from the read-only source dataset.
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
