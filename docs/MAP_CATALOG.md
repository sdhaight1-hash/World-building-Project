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

The additional **Monthly air temperature**, **Monthly precipitation**, **Annual air temperature (duration-weighted mean)**, and **Annual precipitation total** themes make **27 map styles** available in the Studio map library. They are separate source fields with units **°C** and **mm/month**; uniform starting values and painted edits are **author specifications**, not generated temperature, humidity or rain. A month exists only after its values are explicitly created or copied. Unsupported months remain unavailable. Both data maps are rendered from the same currently selected month in the 2D atlas and wgpu globe.

The new `DatasetRegistry` stores field IDs, names, typed units, validation intervals, raster dimensions, explicit `authored/imported/derived` provenance declarations, dependency references, and 1–12 keyed temporal slices. Only the selected month's two source grids are materialized; the project stores a compact baseline + sparse edited-cell overlay. The dataset dependency graph rejects missing references, cycles, mismatched grid dimensions and invalid numeric cells before activation. The renderer exports the declared provenance and time slice into the PNG metadata sidecar.

A dependency declaration describes *what the field claims to depend on*. It does not evaluate a scientific equation. Only explicitly authored values can currently be created in the Studio UI. A future coupled climate model must add an actual physical operator, validated quantity conversions and conservation/energy gates before marking its output `derived`.

**Planet calendar and derived solar map:** world projects now persist an editable 12-month calendar, with individually specified solar-day counts, solar-day duration, obliquity, year-start solar longitude, and stellar flux. The new **Monthly top-of-atmosphere sunlight** map (W/m²) computes daily-mean circular-orbit irradiance at the raster latitude, then integrates across the selected month's duration. It is longitude-invariant, is displayed in atlas/globe and exported with the calendar, month, source lineage and operator. This is orbital geometry, **not** a coupled climate model, actual cloud cover, surface temperature or rain.

Annual temperature requires all twelve source months and weights each month's supplied mean by its specified solar-day duration. Annual precipitation still sums twelve monthly totals. No missing month is inferred. Defaults are explicit fictional **360-day circular orbit** assumptions. Eccentricity, precession, real orbital fitting and climate coupling are future work. The twelve familiar month names are UI indices, not an Earth calendar imposed on the fictional world.

## Atmospheric source inputs and thermal storage (implemented source authoring, not a solver)

The 24-style map library includes monthly **surface pressure** (hPa, 100–1200), **specific humidity** (g water vapor/kg moist air, 0–60), and two **signed horizontal wind components** (geographic east-positive and north-positive, each −150 to +150 m/s). All four use the same sparse month-keyed registry, declared `authored` provenance, validation and editor painting as monthly climate sources. There is no inferred pressure gradient, humidity balance or circulation; absent months remain absent. Their views and exports contain time slices and units; the two wind components are not a solved wind field.

**Surface areal heat capacity** (MJ/(m²·K), 0.01–2000) is a separate time-independent, sparse, non-destructive source layer. The workshop offers illustrative land-like (2) and ocean-mixed-layer (200) brush value presets. Authors must choose which areas are land/ocean and supply the relevant effective layer depth; no landscape classification or thermal budget is calculated. Original geology, temperature, rainfall and solar maps are unaffected. Numeric authorship is not scientific prediction.

## Coupled moist-air and vapor mass-continuity diagnostic (implemented, not weather)

Three opt-in outputs were the first diagnostics in the **27-theme** library: **Transported vapor-column proxy** (kg/m²), **Pressure after transport · diagnostic** (hPa), **Humidity after transport · diagnostic** (g/kg). They require four independently authored monthly sources: surface pressure, specific humidity, and eastward/northward winds. The experiment also takes the planet radius (km) and surface gravity (m/s²) from the saved world recipe. Old projects default to Earth-like gravity 9.80665 m/s² without changing authored data.

Hydrostatic total moist-air mass per area is `M=p_Pa/g`; a vertically uniform specific humidity fraction `q` produces the vapor-column proxy `W=Mq`. A first-order upwind spherical finite-volume operator moves **both M and W with exactly the same face fluxes**. Pressure and humidity outputs are diagnosed as `p=gM` and `q=W/M`. Face transfers are pairwise conservative across periodic longitude and closed polar boundaries, with true spherical cell-area weights, correct mirrored-x geographic wind sign, and an outgoing Courant bound of 0.75. The strict diagnostics report separately the before/after total air mass, water mass, relative residuals, substep counts and ranges **after float32 map projection**. Both residuals above 1e-6 are errors.

**Every map is read-only and ephemeral.** Derived IDs are reserved from the editable monthly dataset registry; computed maps and budget do not persist in `world.json`. Their PNG metadata carries the common operator, dependency IDs, planetary parameters and mass budgets; the budget can be exported as JSON. Editing source fields, changing selected month or planetary geometry invalidates the preview. No authored temperature, pressure, humidity, wind, rainfall or terrain input is overwritten.

**Explicit limits:** Pressure changes solely from divergence of **prescribed** winds; wind momentum, Coriolis/pressure-gradient forces, vertical transport and stratification, dry-air thermodynamics, surface energy and ocean interactions, evaporation, condensation and rainfall are not solved. The column's vertically uniform humidity is a transparent numerical assumption, not predicted precipitable water. Physical climate realism requires a mass-consistent momentum and energy model plus separately verified phase-change/water budgets.

## Experimental pressure-gradient + Coriolis wind tendency (implemented opt-in dynamics preview)

Two additional **derived** wind-component maps (**Wind after dynamics · eastward / northward**, m/s) expand the library to **29 themes**. The new mode uses the same four monthly authored sources as the coupled air/vapor mass operator, and computes horizontal wind tendencies in cell-centered coordinates with **an assumed equivalent barotropic wave speed** `c`, the pressure-gradient term `-c² ∇ln(p)`, and an exact local Coriolis rotation `f=2Ωsin(lat)`. Ω is inferred from the author's mean **solar day** and **orbital year** for an explicitly assumed **prograde circular orbit**. The default effective c=80 m/s is an authored experiment parameter, not an atmospheric sound speed or simulated temperature, and is saved with the project (legacy projects default to 80 m/s).

At each bounded substep, winds are advanced from local pressure gradients and rotation; those evolving winds transport both moist-air mass and water-vapor mass with the same spherical conservative finite-volume flux. The usual spherical cell areas, Aegaea reversed-x/east sign, longitude wrap and closed pole boundaries still apply. The dynamic integration respects outgoing and effective wave-speed CFL restrictions with a hard 512-step limit. Conserved global air and water mass are checked on the five published float32 rasters, with independent relative tolerances of 1e-6. Diagnostics expose inferred sidereal rotation period, peak wind, maximum pressure acceleration, and wave/transport Courant alongside mass budgets. The original **fixed-wind mode** remains independently accessible; switching modes cleans up stale wind diagnostics.

The new maps are opt-in and **ephemeral**; derived dataset IDs are reserved against authored values, and all five outputs are invalidated when monthly inputs, calendar, gravity, radius, or effective c change. PNG/JSON exports identify the chosen operator and assumptions. **Not physically complete:** cell-centered tendencies omit nonlinear momentum advection, spherical metric/curvature terms, vertical atmosphere, energy and angular-momentum closure, friction/forcing, cloud phase changes and precipitation. Retrograde spin is not modeled. Full spherical rotating shallow-water benchmarks such as the [Williamson et al. 1992 test suite](https://www.sciencedirect.com/science/article/pii/S0021999105800166) are not yet passed or claimed.

## Spherical absolute axial momentum finite-volume preview (implemented, opt-in)

The **Compute conservative spherical momentum preview** action is a distinct third solver mode; it publishes the same five ephemeral derived themes, so the library remains **29 thematic maps**. It transports total moist-air mass, water vapor and **absolute axial angular momentum** `M=a cosφ (u+Ω a cosφ)` in the same first-order spherical finite-volume face exchanges, with paired antisymmetric east-west barotropic pressure torques. Meridional momentum is donor-cell transported with the appropriate nonlinear spherical curvature and Coriolis source, plus an explicit latitude-face pressure-force geometry term designed to cancel exactly in a uniform resting state. The closure `P=c²h` assumes a two-dimensional barotropic column of mass per area `h=p/g`; it does not reconstruct temperature, vertical density or actual moist atmospheric pressure physics.

Pole boundaries are true zero-length spherical cap edges, rather than arbitrary finite rectangular edges; spherical band areas and exact meridional-integrated axial moment arms set the flux and pressure torques. The near-pole longitude cell length controls the wave/flow CFL, and expensive intervals are refused before executing hundreds of unsafe steps. Signed east/north geographic wind orientation uses the shared reversed-x atlas convention. The global axial momentum, air and vapor residuals are audited from the **published f32 maps** and must each be ≤1e-6 relative. Additional diagnostics report the kinetic + relative barotropic compressional energy before/after, signed energy drift, pole boundary flux, wave/outflow Courant values and minimum polar cell width. **Energy conservation is not claimed:** first-order donor-cell transport dissipates energy.

All five maps retain scientific operator, selected month, assumptions, planetary settings and numerical diagnostic budget in native PNG metadata; they are read-only and cleared when the sources or planet settings change. Full spherical vector momentum, pole-crossing treatment, energy-preserving time integration, rigorous global angular momentum and standard shallow-water benchmarks, multi-layer thermodynamics, ocean, clouds and precipitation remain open engineering tasks. Background reference: [NOAA/GFDL MOM3 description of axial momentum conservation](https://www.gfdl.noaa.gov/wp-content/uploads/files/model_development/ocean/mom3_manual.pdf).
