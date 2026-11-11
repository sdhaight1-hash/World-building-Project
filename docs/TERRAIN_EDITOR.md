# Non-destructive terrain editor — MVP

## What is supported

- **Create flat ocean DEM** for a blank world (initial -2,000 m).
- **Raise / lower** geographic elevation with a falloff brush (radius in source grid cells; strength in metres).
- Paint in 2D or on the wgpu 3D globe; source coordinates remain mirrored and inverted, longitude wraps.
- Click or drag, with one undoable command per drag. Ctrl+Z / Ctrl+Y, plus toolbar controls.
- Save sparse absolute height overrides in \`Project.terrain\`, separate from imported TIFF source files.
- Rebuild visual terrain hillshade, contours, rain-shadow exposure, and displaced sphere on gesture completion.
- Export an authored 32-bit floating-point elevation TIFF and companion provenance JSON.

## Invariants

Source science data is **immutable**. An override is not a modeled geological process. The terrain brush neither rotates tectonic plates nor moves craton boundaries, so users must reconcile any intentionally edited landforms with geological interpretation separately. Rain-shadow exposure is recalculated because it depends on elevation; its physical limitations have not changed.

The project's \`terrain.grid_size\` blocks applying overlays to grids with different dimensions. The editor presently does not hash the source raster contents, so replacing the underlying DEM with a same-sized but unrelated one is unsafe and must be addressed before general source remapping. Reversible brush actions operate on the in-memory edited raster and restore the previous override state.

## QA

- Paint / undo / redo results are identical; grouped dabs undo all the way to original elevations.
- Brush wraps longitude on both atlas edges.
- A saved project overlays identical values on a clean source DEM.
- Blank-world ocean initialization persists across reopening.
- Numeric exported TIFF round-trips cell values exactly.
- No input TIFF or Git data file is overwritten.
- Desktop visual acceptance: inspect altered relief in 2D and globe, confirm rain-shadow changes, save and relaunch, then undo a fresh edit.

## Limits

No raster tiled LOD, autosave, coast-preserving geological editing, erosion simulation, layered terrain modifiers, polygon constraints or brush radius measured in geodesic kilometres yet. The current distance falloff is in raster cells and visually distorts toward the poles. Rebuilding dependent maps synchronously after a stroke can pause interaction on very large worlds. These are explicit future stages, not features claimed complete.
