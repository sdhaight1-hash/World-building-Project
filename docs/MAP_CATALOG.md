# Physical-world map catalog (planned)

The catalog describes **map products and their input data**, not a list of magic one-click simulation buttons. All map families permit import, manual authoring and visual analysis even if a computational generator does not yet exist.

| Family | Data layers | Useful map products | Typical dependencies |
| --- | --- | --- | --- |
| Planetary | radius, orbital period, obliquity, insolation | day length, stellar flux, latitude/season insolation | orbital parameters |
| Tectonics | plate IDs, Euler rotations, ridges, trenches, cratons, faults | plate map, paleo-continent maps, strain map, crust age | plate geometry + kinematics |
| Geology | formations, basement age, lithology, stratigraphy, heat flow | bedrock age, rock class, metamorphism, sedimentary basins | crust history + depositional settings |
| Terrain | DEM, sea level, land mask, slope, aspect | elevation, relief, contours, landforms | tectonics + erosion/authoring |
| Bathymetry | shelf break, basin depth, crust age | bathymetry, ocean-depth contours, ocean-floor age | crustal structure + sea level |
| Oceanography | current vectors, SST, salinity, sea ice | seasonal currents, upwelling and marine heat | atmosphere + basin geometry |
| Atmospheric | wind vectors, surface pressure, humidity | prevailing winds, pressure and moisture | insolation + terrain + rotation |
| Climate | monthly temperature/precipitation | annual precipitation, Köppen, aridity, seasonality | atmosphere + ocean + terrain |
| Rain shadow | elevation DEM, incoming wind bearing, terrain exposure index | signed windward/lee shadow map and optional seasonal composites | terrain + wind direction; actual precipitation needs moisture/climate |
| Hydrology | runoff, accumulation, drainage, basin boundaries | watersheds, river networks, lakes, wetlands | DEM + precipitation + geology |
| Soils | parent material, moisture, erosion, fertility | pedological units, horizon/soil series, drainage | lithology + climate + organisms |
| Ecology | vegetation, temperature tolerance, water demand | biome, forest cover, habitats | climate + soils + terrain |
| Resources | deposits, rock class, geothermal fields | mineral potential, fuels, salt/evaporites | geology + climate + time |
| Hazards | stress proxy, fault motion, saturation, storm forcing | quake/eruption likelihood, flood/slide susceptibility | appropriate verified fields |

## Layers vs maps

One formation polygon may participate in bedrock-age, lithology, resource, and tectonic history maps. The **stored map** contains display settings; the geology feature remains a single data record. Preserve category codes and scientific descriptions in the legend.

## Aegaea v1 baseline

- Available: a native 1672x941 grid, 17 plate IDs and Euler parameters, 13 craton IDs/ages, plate-contact linework, crust/province/rheological assumptions and an initial DEM.
- Unsolved: subduction polarity; backward plate topology; detailed stratigraphy; true mineral occurrence; dynamic ocean ages; coupled thermal/erosion/climate model.
- Status of any derived layer must say **modeled/assumed** where appropriate.

## Dedicated rain-shadow map (implemented terrain-only P0)

The standalone **Rain Shadow** thematic map takes a DEM and a user-selected wind direction and calculates a dimensionless signed exposure index. Blue is orographic/windward uplift potential; red is leeward barrier shelter. Displayed and sampled in both map projections; exportable as a floating-point TIFF + metadata JSON. It does **not** predict precipitation totals. See [the scientific and engineering contract](RAIN_SHADOW.md).

## Surface slope (implemented map)

The **Surface Slope** map is a scalar raster computed from the elevation DEM by the same spherical gradient operator in `map_graph.rs`. Units are **degrees**, with latitude-aware physical distances between raster cells, wraparound at the longitude seam, and an explicit palette/legend. Available interactively in the atlas and true mesh globe, inspectable at a selected location, and exported as PNG/SVG by the existing map pipeline. It automatically updates following authored DEM edits. The reference spherical radius is 6,371 km and must be parameterized in future arbitrary-planet projects.

## Euler plate-contact velocity maps (implemented)

The source `plates.csv` declares each Aegaea plate's signed Euler rotation rate and pole. Studio computes three independent diagnostic scalar layers at *actual present plate-ID contacts*: **Plate convergence** (positive closing component), **Plate spreading** (positive opening component), and **Plate shear** (tangential relative motion). All use **cm/year**, spherical edge-length weighting, 3D unit vectors, seam-safe indexing, and the dataset's transform-dominance threshold. These are not distances in pixels, abstract terrain roughness classes, or elevation predictions.

The active atlas and wgpu globe use the same newly generated numeric fields, with subtle source-relief context, distinct palettes and five-step legends. The inspector reports the raw derived value at the selected coordinate; `--export-maps` produces native-resolution PNG and publication SVG sheets for all three. For blank worlds or imports without Euler motion metadata, these maps are unavailable rather than silently synthesized. Existing geological source TIFFs are never rewritten.

**Interpretation:** The values are localized at sampled plate contacts. A zero may indicate no contact, a transform-dominated boundary, or a zero component; it is not proof of geological inactivity. No subduction polarity, deep-time integration, plate reconstruction, erosion or earthquake rate has been inferred.
