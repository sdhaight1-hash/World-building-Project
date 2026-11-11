# Architecture and data contract

## Technical starting point

- Rust (2021 edition) and `eframe/egui` as the desktop shell and docking/inspector foundation.
- `wgpu` backend for rendering; a later purpose-built 2D/3D renderer will use custom WGSL pipelines.
- Small, testable geospatial domain modules distinct from UI.
- On-disk source datasets (GeoTIFF, GeoJSON, CSV) remain immutable imports; edits go to an independent project file/overlay. SQLite is a proposed later project index once editing goes beyond annotations.
- No blanket choice of an Earth coordinate reference system for a fictional planet.

### Coordinate and sampling invariants

The imported Aegaea raster is native **1672x941**; its *fictional* geographic mapping, at pixel centers:

```text
longitude(x) = +180 - 360*(x + 0.5)/width
latitude(y)  =  -90 + 180*(y + 0.5)/height
```

The view should intentionally show an inverted North/South and left/right-mirrored world. `longitude` decreases to the right. Do **not** assume standard north-up rasters or silently attach Earth EPSG:4326.

The two views use the **same** sampling function; no separate hand-painted globe texture. Longitude wraps periodically; latitude clamps to the supported range. Unmapped and nodata categories must be handled explicitly.

### Data types (target model)

- `PlanetDefinition`: radius, rotation, reference frame, datum, authored metadata.
- `LayerId`: stable key, category, version.
- `RasterLayer`: grid, band count, data type, nodata, units, time, source reference, expression lineage.
- `VectorLayer`: point/line/polygon feature collection, attribute schema, CRS, topology rules.
- `MapStyle`: classification method, color ramp, boundary stroke, opacity, symbol and legend templates.
- `SavedMap`: viewport, projection, visible layer stack, styles, annotations and export layout.
- `Provenance`: source, parameters, tool version, random seed, uncertainty and dependency hashes.
- `EditRecord`: input action + inverse/undo + target + timestamp.
- `TimeAxis`: geological Ma, calendar times, monthly climatologies; never conflate these.

### Data flow

```text
Source files (immutable) --> Normalized fields --> Analysis/generators --> Derived fields
                                  |                    |
                       Authored edit overlays <--------+
                                  |
                      Saved map compositions
                         /            \
                    2D atlas       3D globe
                         \            /
                       Exporters and reports
```

Source/dependent invalidation must be targeted. Editing a river must not force regeneration of tectonic plates. Users choose when to recompute generated layers.

### Rendering implementation phases

1. **Bootstrap (this branch):** egui painters + software-computed atlas/globe textures running through wgpu to prove identical geospatial coordinates and data probing. Not a native 3D terrain mesh. Features can be inspected; early marker editing is minimal.
2. **Production 2D:** GPU rasters, tile caches, vector tessellation, map projections, brush editing, contours/isolines and semantic sampling.
3. **Production 3D:** wgpu vertex/index terrain meshes displaced by heightfields, ellipsoid mapping, adaptive LOD, GPU resource paging, world-space picking and physically coherent lighting. **No voxels.**
4. **Map compositor:** dynamic legends, multi-layer styling, layout templates, publication-size output.
5. **Physical map generators:** explicit standalone computation modules with controlled input/output contracts and validation.

### File/data policy

The repo has unusually large pre-existing scientific imports split into `.binary-parts`. The desktop slice uses ordinary checked-in plate/craton/province GeoTIFFs and their CSV metadata. `elevation_m.tif` is split; the loader now assembles its committed parts **in memory** when the TIFF is absent, so users do not need to run Python to obtain the source DEM. The app must report corrupt/missing elevation, not synthesize its values.

Scientific status: Aegaea v1 plates are a synthetic *instantaneous* Euler configuration, cratons are assigned ancient blocks, and deep-time history is hypothetical. No implementation should claim complete tectonic physics.


## Native data-driven cartography (P0.2)

The `cartography.rs` module builds an immutable rendering cache from the geological rasters and exact classified plate-boundary linework in `boundary_segments.geojson`. It produces physical hillshade and coastline masks from the original DEM, plate/craton/province boundaries from category changes, and line geometry from source vector coordinates. Palette keys for plates use the actual `plates.csv` colors. Province category labels come from `crustal_provinces.csv` rather than arbitrary index strings.

`render.rs` projects this same classified field onto the 2D native atlas and an orthographic spherical preview. The **UI and export paths share the exact same map palette and legend generator**. PNG outputs are accompanied by JSON legends and metadata. This replaces the old 1024-wide and 512-wide downsampled previews (globe is now 896 px).

This is **not** a new terrain generator, fully reconstructed geological simulation or textured 3D terrain mesh. The physical relief comes from Aegaea's existing synthetic elevation DEM. The cartographic output includes modeled geological interpretations and should be labeled accordingly.
