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
