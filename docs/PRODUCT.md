# Product brief — Aegaea Studio

**Status:** agreed direction; initial vertical slice in `studio/`. **Audience:** creative planetary cartographers, geofiction writers, physical-world designers, speculative scientists, and TTRPG/novel authors who need *maps of the natural world*, not campaign or civilization management.

## North star

A unified **physical-world map and data creation environment**. Users may construct maps by manual editing, assisted calculations, importing scientific data or optional procedural generation. A first-class shared data model supports viewing the same terrain and attributes in 2D and on a 3D globe; multiple maps may style the same data differently.

**Aegaea is both the software name and the reference fictional world.** The studio must not bake Aegaea-specific shape, plate count or coordinate parameters into its core.

## Key entities and operations

- **Planet:** radius, orientation, map datum, day/year, obliquity, optional stellar flux, natural environmental constraints.
- **Dataset:** raster grid, vector collection, numeric timeseries, categorical map, geometry/volume/section or tabular metadata; with units, provenance, uncertainty, temporal extent, resolution, coordinate system.
- **Map:** saved composition of layers + style rules + projection + labels + layout and export settings.
- **Feature:** a geospatial point, line, polygon, profile or 3D geological surface with stable ID, typed fields, scientific narrative, relationships to other *physical* features.
- **Operation:** explicit, reproducible edit or derivation of output fields from input data, with parameter versions and manual masks/overrides.
- **Snapshot:** a time-specific state; distinguish historical reconstructions from current snapshots and from speculative scenario timelines.

## Map families

1. Planetary setting and insolation (star and orbit are parameters, not a required full galaxy simulator).
2. Tectonics and deep-time reconstruction (plates, cratons, faults, sutures, ridges, subduction).
3. Surface/subsurface geology (age, lithology, formations, sedimentary basins, cross-sections).
4. Terrain/bathymetry (elevation, hypsometry, geomorphology, sea level).
5. Oceanography (currents, temperature, upwelling, salinity, ice).
6. Atmospheric dynamics (wind, pressure, moisture).
7. Climate (seasonal temperature, precipitation, climate zones).
8. Hydrology/cryosphere (rivers, watersheds, lakes, glaciers).
9. Soils, ecological/biogeographic patterns.
10. Resources and physical hazards (mineral potential, volcanism, seismicity, landslides, drought).

**Explicit non-goals:** voxel terrain; civilizations, political boundaries, demographics, trade, factions, character sheets, world-bible/wiki replacement, campaign management, speculative settlement modeling, full stellar-neighborhood simulation.

## Authoring experience

A scientist can import data and use attribute tables, legend rules, measurement and sampling. A novelist can paint plausible (or deliberately impossible) physical geography and attach scientific notes. A single dataset may feed many map styles: for example bedrock geologic-age colors versus rock-family colors. Information is never flattened into a picture until exported.

User must be able to override a derived feature and protect it from regeneration. Inconsistency warnings are useful; forced 'realism' is not.

## Acceptance of product direction

- A new project can be created with **zero** reference-world data.
- Aegaea can be opened and inspected with no code modifications or special enumerations.
- Editing data in 2D is reflected in 3D and vice versa.
- A saved map can be reopened with identical source references and styling.
- A physical feature can carry meaningful numeric attributes with units and provenance.
- A finished atlas page can be exported with title, key, scale, coordinate/grid information and bibliography.
- A generator distinguishes assumptions, numerical computation, and validated physical simulation.

Items above are product acceptance goals, **not all implemented** by the first vertical slice.
