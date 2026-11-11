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

## Independent-world authoring for the original fourteen map styles

A new blank world does not inherit Aegaea's geology. Select any style in **Map library**, including those labeled *needs source*, and use **Map Layer Workshop** to create and paint the input fields. Six input families are stored as sparse project-local typed rasters, with explicit initial author assumptions and editable records for named entities. The `world.json` overlay is separate from immutable source TIFFs; the same authored fields render in the atlas and displaced globe.

| Map styles | Source data users author | Actual computation / caveat |
| --- | --- | --- |
| Physical relief/bathymetry | flat-ocean DEM + Terrain brush | user-authored; generation from crust/province/plates only when prerequisites met |
| Surface slope | DEM | calculated spherical gradient |
| Tectonic plates | plate IDs and named plate records | painted identities, not reconstructed plates |
| Plate boundary classes | plate ID contacts; imported or **two-click manually authored** classed contact lines | geometry from painted IDs; unclassified until an author assigns a contact class, no automatic subduction |
| Plate convergence, spreading, shear | plate IDs + complete signed Euler pole/rate per plate | instantaneous relative motion, not seismic rates or reconstruction |
| Cratonic basement | craton IDs and named records with parent plate | painted categories, consistency diagnostics available |
| Basement age | painted craton regions + user-specified basement age in Ga | attributed from records, **not** radiometric simulation |
| Crustal provinces | province IDs 0–10 | class IDs follow the current terrain-generation v1 scheme |
| Crust thickness | numeric km raster | author-assumed or imported, not inferred from elevation |
| Lithosphere depth | numeric km raster | author-assumed or imported |
| Surface heat flow | numeric mW/m² raster | author-assumed or imported |
| Rain shadow | DEM and chosen incoming wind direction | signed orographic exposure proxy, not monthly rainfall |

**Boundary authoring:** Select **Plate boundary classes**, initialize **Plate IDs** if needed, then use **Classified contacts → Draw classified contact (two clicks)**. Choose subduction (polarity unspecified), continental collision, divergent spreading, or transform; click start and end in either projection. These are user assertions stored in `world.json`, not deductions from plate IDs. Unclassified contacts retain their own neutral legend until a classified record exists. Use **Remove last contact** to retract the most recent user segment.

**Integrity constraints:** Categorical paint values must match named records, scalar paint values have finite domain bounds, painted changes can be undone and persisted, and created/source rasters retain fixed project resolution. An unclassified boundary or absent Euler rotation stays unclassified/unavailable. All mapped values remain explicitly distinguished from modeled science. Future ocean, atmosphere, monthly climate, hydrology, soils, ecosystems, resources and hazards require new **typed source layers** plus separate derivation operators and cannot be provided by relabeling unrelated rasters.

## Monthly climate authoring — first typed scientific dataset extension

The additional **Monthly air temperature** and **Monthly precipitation** themes make **16 map styles** available in the Studio map library. They are separate source fields with units **°C** and **mm/month**; uniform starting values and painted edits are **author specifications**, not generated temperature, humidity or rain. A month exists only after its values are explicitly created or copied. Unsupported months remain unavailable. Both data maps are rendered from the same currently selected month in the 2D atlas and wgpu globe.

The new `DatasetRegistry` stores field IDs, names, typed units, validation intervals, raster dimensions, explicit `authored/imported/derived` provenance declarations, dependency references, and 1–12 keyed temporal slices. Only the selected month's two source grids are materialized; the project stores a compact baseline + sparse edited-cell overlay. The dataset dependency graph rejects missing references, cycles, mismatched grid dimensions and invalid numeric cells before activation. The renderer exports the declared provenance and time slice into the PNG metadata sidecar.

A dependency declaration describes *what the field claims to depend on*. It does not evaluate a scientific equation. Only explicitly authored values can currently be created in the Studio UI. A future coupled climate model must add an actual physical operator, validated quantity conversions and conservation/energy gates before marking its output `derived`.

**Calendar caveat:** the first temporal interface uses twelve named Earth-calendar months as an indexing convention; arbitrary planet day/year length, seasonal forcing and length-weighted statistics are future features. No Köppen class or annual climate is inferred from incomplete monthly data.
