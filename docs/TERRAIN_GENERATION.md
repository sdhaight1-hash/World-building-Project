# Geological terrain synthesis v1 — source-independent DEM

## Architectural change

**The legacy elevation image is not the world model.** When an Aegaea project has geological fields and no older brush overrides, Studio now defaults to creating terrain from the typed fields, not displaying \`elevation_m.tif\`. The imported DEM remains available via **View legacy DEM** for comparison. Both use the same downstream renderer, 3D globe, slope, rain shadow, and exports.

The new module \`studio/src/terrain_generation.rs\` takes the geological scenario inputs:
- **Province categories** 0–10 define explicit ocean/continent and tectonic domain masks.
- **Crust thickness (km)** contributes a terrain-uplift proxy on continental domains.
- **Plate ID adjacency** supplies a distance-to-current-boundary proxy to increase terrain complexity nearby. The implementation does **not** infer true convergence/extension vectors.
- **Craton masks**, if available, keep core terrain smoother and more resistant to arbitrary detail.
- A configurable seed, relief strength and planet radius control deterministic multiscale variation in spherical 3D coordinates.

The code deliberately **does not read any original elevation data**, and regression tests compare a world with no elevation to one whose elevation is replaced with absurd numbers, requiring bit-for-bit identical generated terrain.

## Actual stages

1. Classify oceanic and continental provinces; validate all input grid dimensions and numeric categories.
2. Measure distance from ocean and land in wraparound grid cells; use this to shape continental margins and continental shelves.
3. Smooth tectonic province masks into regional forcing fields (mountain belts, arcs, rifts, spreading ridges and trenches).
4. Build spherical, longitude-seam-safe, latitude/pole-continuous multiscale noise from unit-sphere coordinates and the saved recipe seed. Apply greater roughness around plate boundaries and less in cratons.
5. Combine continental crust thickness, regional forcing, seafloor depth, shelves, and terrain variation into a **new signed elevation field**. Coastal provinces retain their original land/ocean identities as an explicit constraint.
6. Export Float32 elevation, tectonic-class forcing, land mask, PNG physical atlas and scientific provenance. Derived surface slope and rain shadow are recomputed using the newly generated heightfield.

This is a **first procedural terrain hypothesis**. It is not solved plate tectonics or validated erosion. The class masks, crust thickness and other source geology were constructed in the previous synthetic dataset and may have been influenced by the legacy terrain generator. That makes this independent of the **height values of the old DEM**, but **not independent of all historical upstream assumptions**. A future iteration must regenerate the geological provinces from plate geometry/kinematics and other independently chosen parameters.

## UX and compatibility

In the sidebar expand **Terrain Source / Generator**. By default an untouched world with enough geological inputs uses **Generated from geology**. **View legacy DEM** switches to the original source raster without changing it. Seed and relief strength can be edited; press **Generate from geology** to apply.

A project with existing brush overrides from the previous version automatically uses **Imported** unless its source choice was explicitly saved. Once edited, the selected terrain basis is persisted in the project JSON. To prevent corrupt reapplication, switching the base while local brush overrides exist is blocked. A project JSON stores source mode + seed/parameters; the generated DEM itself remains reproducible and can be exported as TIFF.

Creating a blank world without geology still works: the terrain generator is unavailable, and the flat-ocean authoring tool remains operational.

## Headless acceptance and outputs

From the repository root:

\`\`\`powershell
cargo run --release -- --generate-terrain maps/geology-generated
cargo run --release -- --export-maps maps/aegaea
\`\`\`

The first command produces \`generated-elevation_m.tif\`, \`tectonic-forcing_m.tif\`, \`geology-land-mask.tif\`, \`generated-physical-relief.png\`, and \`generation.json\` (plus the PNG legend JSON). The second exports all enabled map categories, now deriving their terrain-based layers from the new generated surface when no legacy edits are present.

Automated Windows checks must demonstrate generator success on **the actual Aegaea source grid with \`World.elevation = None\`**, deterministic replay, seed variation, pole convergence, positive land/negative ocean values, generated slope and rain-shadow calculations, numeric round-trip, and outputs from the CLI.

## Explicit limitations and next scientific stages

- Province ID 0–2 / 3–10 acts as an author-controlled land/ocean constraint, including shapes inherited from an earlier design. This may produce coastline similarities to the original map.
- The tectonic force amplitudes, crust uplift scaling, shelf widths and detail parameters are **heuristic**, not physically calibrated rates, a stress field, or an isostatic gravity solver.
- Region distances are presently measured in **raster cells**; spherical latitude-aware physical distance and polar cross-seam geodesics need a dedicated solver.
- No dynamic uplift, plate-pair convergence vectors, trench polarity, ocean crust age solution, erosion, drainage, sedimentation, or water conservation.
- The native global grid is only 1672×941. It cannot resolve true small river valleys or fine coastal geomorphology. Regional multiscale refinement must generate new consistent detail and not mislabel interpolated source pixels as observations.
- The source atlas's actual geology/terrain dependencies and quality must be audited before replacing source provenance or claiming tectonic realism.

The next milestone is **independently rebuilding the province/tectonic forcing geometry from plate motion and crustal properties**, then validating regional shapes in recorded desktop views before advancing erosion.
