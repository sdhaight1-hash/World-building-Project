# AEGAEA — Comprehensive tectonic, crustal and cratonic synthesis

**Dataset version:** 1.0 (fictional atlas). **Native data grid:** 1,672 × 941 cells. **World:** fixed inverted/mirrored coastline. **Model type:** designed conditional tectonic scenario, not a coupled geodynamic or time-backward reconstruction.

## Executive geological interpretation

This fictional planet's present-day lithosphere consists of 17 instantaneous rigid Euler plates and 13 distinct ancient cratonic basement domains. Unlike copying Earth's known plates, the plate footprints are generated from an original warped seed system, each plate has a synthetic Euler pole, and each contact is classified from instantaneous relative kinematics. The ancient basement cores are clipped to their current plate and land masks. The original synthetic relief is preserved exactly. A second suite of *scenario* fields adds Moho depth, lithosphere–asthenosphere boundary (LAB), surface heat flow, and tectonic province classes.

The first two facts are numerical consequences of *our fictional source map*, not planetary observations. A crustal shield polygon, for example, is an assigned Archean core with implied durable lithosphere—not evidence that geological processes in the past really produced it. A cratonic age is a selected model age, not a radiometric measurement.

The planet radius is **6,371 km**, chosen only to establish sensible velocity, area and length units. The land-mask classification yields **206.34 million km² land (40.45% of its surface)**. This number is conditional on image-color extraction: it is not a reliable replacement for a raster land/sea survey. It is larger than Earth's familiar 29%-land fraction partly because this is a reworked illustrative atlas.

The total assigned cratonic area is **65.17 million km² (31.6% of detected land)**. These are *current exposed land* craton-core areas only; rifted offshore fragments or buried cores under continental margins have not been modeled. Their cratonic basement ages span 2.58–3.45 billion years.

## 1. Spatial reference and physical basis

The map is not a conventional north-up east-right terrestrial map. In atlas pixel coordinates, x increases to the right as longitude *decreases*, and y increases downward as latitude *increases*. Thus the top of the map is the south pole and its right side is geographic west. Longitude wraps at the left/right map seam. Convert pixel indices to the model's **fictional atlas** angular coordinates before ingesting into a globe shader or physics engine. GeoTIFF files intentionally have no EPSG CRS because attaching a real-world CRS would invite misleading geographic overlays. The 8K images are upscaled presentations; all numerically distinct data come from the source grid.

The 17 Euler rotations are expressed as pole longitude, pole latitude, and signed rotation angle per million years. Surface rigid-motion velocities satisfy `v = omega × r`. Relative normal motion between neighboring plate cells is divided into spreading, convergence, and transform motion. In a dominantly continental neighborhood convergence is labeled collision; otherwise it is labeled subduction. This simplified rule cannot identify slab polarity or trace oceanic microplate fragments, and may misclassify near shorelines and triple junctions.

The length of a plate contact is calculated by summing its north–south or east–west adjacency edges using the planet's spherical metric. It is *not* the length of a physically smoothed fault centerline. Values are therefore precise to the algorithm, not accurate geological measurement.

## 2. Inventory of all 17 modern plates

A positive Euler rate follows the input reference-frame convention and a negative rate reverses rotation. The maximum and weighted-mean plate speeds are in cm/year in the supplied arbitrary absolute reference frame, not toward/away from a neighbor. Plates can include both continental and oceanic crust.

| Plate | Area (Mkm²) | Land (%) | Pole (lon,lat) | Rate (°/Ma) | Mean speed (cm/yr) | Cratons |
| --- | --- | --- | --- | --- | --- | --- |
| NIVALE | 9.68 | 34.5 | (25, -79) | +0.31 | 0.85 | — |
| KORU | 35.30 | 28.9 | (90, 25) | +0.44 | 4.67 | KAVRA |
| LYRAN | 45.60 | 45.5 | (-80, -30) | -0.40 | 3.68 | ORVEL |
| VEYRA | 43.15 | 49.7 | (70, -39) | -0.55 | 5.08 | — |
| SEREVAN | 28.27 | 75.0 | (-55, 62) | +0.43 | 4.02 | MAREN;KEVAR |
| BRINEVEIL | 25.09 | 46.0 | (-20, 0) | -0.37 | 3.53 | ILTHEN |
| ANKAR | 18.67 | 0.6 | (60, 12) | +0.41 | 4.18 | — |
| MERROW | 52.75 | 65.6 | (-10, -55) | +0.49 | 3.81 | SYRIN;EIDEN |
| THALASS | 32.97 | 49.8 | (-45, 5) | -0.51 | 3.45 | NALOR;FARON |
| CAERIN | 22.86 | 14.1 | (140, -20) | +0.38 | 4.00 | — |
| RUSKEL | 29.29 | 16.6 | (95, 40) | -0.43 | 2.74 | — |
| ORREN | 35.04 | 33.4 | (-25, -25) | +0.47 | 5.06 | TAVEN |
| HALDEN | 35.52 | 55.5 | (20, 49) | -0.58 | 5.86 | SELDAR |
| VALLIN | 25.21 | 63.7 | (75, -23) | -0.36 | 2.44 | HERIN;VORUN |
| AUREOLE | 35.42 | 15.0 | (-110, 58) | +0.51 | 4.52 | — |
| DRAEM | 4.82 | 47.0 | (-5, -61) | +0.34 | 2.76 | — |
| SUNDRAL | 30.44 | 11.9 | (130, 3) | -0.35 | 2.85 | — |

**Interpretation:** Merrow, Lyran, Veyra and Halden contain extensive or medium-sized land portions and can sustain long-lived subduction-accretion or collision effects at their margins. Ankar, Caerin, Ruskel and Sundral are primarily oceanic within this source mask. Nivale is a partly land-bearing south-polar cap. These are present-day composition labels, not permanent identities; a craton can survive the eventual breakup of the plate currently transporting it.

## 3. Quantitative plate-boundary budget

The following lengths are the sum of individual classified contacts across the entire native grid. A *single plate pair may appear in more than one class* because contacts curve, pass onto land, or change local relative velocity along their trace.

| Boundary | Length (km) | Share (%) | Contact records |
| --- | --- | --- | --- |
| subduction | 64,498 | 27.5 | 31 |
| continental collision | 28,120 | 12.0 | 22 |
| divergent spreading | 97,676 | 41.6 | 40 |
| transform | 44,630 | 19.0 | 27 |

Total contact length approximately **234,924 km** across 120 plate-pair–boundary-class records, represented by 4,681 trace segments. These represent Euler-kinematic contact classes rather than fully strain-balanced fault zones.

### 3.1 Longest inherited model contact systems

**Convergent (putative subduction) systems** have a trench/forearc/arc/retroarc architecture only if oceanic lithosphere exists on the descending side, a polarity is chosen, and long-term convergence persists. Without polarity, the model can identify a possible zone, **not which side carries the volcanoes**.

#### Candidate trenches and volcanic arcs
| Contact | Length (km) | Rel. speed (cm/yr) | Normal rate (cm/yr) |
| --- | --- | --- | --- |
| VEYRA / SUNDRAL | 11,782 | 5.17 | -3.52 |
| HALDEN / AUREOLE | 5,154 | 9.96 | -9.44 |
| KORU / ORREN | 4,560 | 8.70 | -6.41 |
| VALLIN / AUREOLE | 4,509 | 2.00 | -1.85 |
| LYRAN / AUREOLE | 4,102 | 7.04 | -6.48 |
| ANKAR / SUNDRAL | 3,627 | 6.08 | -4.46 |
| CAERIN / RUSKEL | 3,409 | 3.38 | -2.59 |
| NIVALE / ORREN | 2,755 | 4.26 | -3.45 |

#### Candidate thick-crust orogens and foreland basins
| Contact | Length (km) | Rel. speed (cm/yr) | Normal rate (cm/yr) |
| --- | --- | --- | --- |
| HALDEN / VALLIN | 4,607 | 6.22 | -3.59 |
| MERROW / RUSKEL | 3,658 | 4.37 | -1.39 |
| LYRAN / SEREVAN | 3,606 | 5.71 | -4.18 |
| LYRAN / VEYRA | 3,007 | 6.79 | -3.64 |
| THALASS / DRAEM | 2,838 | 7.62 | -6.92 |
| MERROW / THALASS | 2,781 | 7.50 | -4.25 |
| ORREN / AUREOLE | 1,340 | 8.62 | -8.42 |
| SEREVAN / BRINEVEIL | 1,124 | 6.70 | -6.05 |

#### Candidate spreading axes and intracontinental rifts
| Contact | Length (km) | Rel. speed (cm/yr) | Normal rate (cm/yr) |
| --- | --- | --- | --- |
| LYRAN / AUREOLE | 11,032 | 7.05 | +3.19 |
| KORU / SUNDRAL | 8,496 | 6.58 | +4.88 |
| ANKAR / MERROW | 8,336 | 6.12 | +4.18 |
| RUSKEL / HALDEN | 7,850 | 4.65 | +2.93 |
| VEYRA / SEREVAN | 7,015 | 2.98 | +2.58 |
| THALASS / VALLIN | 5,700 | 5.90 | +5.39 |
| BRINEVEIL / DRAEM | 4,121 | 6.52 | +6.37 |
| SEREVAN / BRINEVEIL | 3,646 | 6.59 | +3.15 |

#### Candidate long-lived strike-slip systems
| Contact | Length (km) | Rel. speed (cm/yr) | Normal rate (cm/yr) |
| --- | --- | --- | --- |
| KORU / LYRAN | 5,502 | 0.92 | -0.04 |
| VALLIN / AUREOLE | 4,266 | 1.54 | -0.51 |
| SEREVAN / BRINEVEIL | 3,063 | 6.09 | +0.23 |
| BRINEVEIL / THALASS | 2,893 | 2.54 | +0.04 |
| CAERIN / ORREN | 2,510 | 8.25 | -1.59 |
| ORREN / AUREOLE | 2,454 | 8.66 | +1.33 |
| KORU / SUNDRAL | 2,306 | 5.59 | +0.13 |
| KORU / ANKAR | 2,255 | 2.24 | -0.05 |

### 3.2 Physical interpretation of boundaries

**Subduction:** plate convergence can remove oceanic lithosphere and transport sediment and hydrated mafic material deep into the mantle. A complete model would solve slab polarity, dip and age, seismic coupling, accretionary-prism sediment flux, hydrated-wedge melting, and arc position ~100–300 km inland from the trench as geometry permits. The present raster only indicates convergence and whether the local land fraction is low.

**Continental collision:** strong crustal shortening leads to thickening, folding, crustal root development and partial melting; erosion redistributes that uplift into large foreland basins. A collision label is plausible only when *both sides* are buoyant continents. The present classification uses a blurred land cover proxy and therefore needs a future crust-type-on-each-side test.

**Spreading/rifting:** divergence creates oceanic crust at spreading centers or thins continental lithosphere until rupture. Mature spreading should be accompanied by age isochrons, a mid-ocean ridge heat-flow anomaly and roughly symmetric young crust on both flanks; all are future validation conditions. A divergent contact inside an ancient craton would require large rift forces or reactivation of older mobile belts, so its role is not accepted blindly.

**Transform:** lateral motion offsets crustal features and accumulates shear strain, with local uplift or pull-apart subsidence at restraining and releasing bends. Slow contacts (less than roughly 1 cm/year) can be effectively low-activity even if they are algorithmically marked transform.

**Triple junctions:** where three plates meet, contact geometry needs a stable topological solution. The simple source Voronoi plate generation does not enforce triple-junction stability; before deep-time evolution, junctions should be screened for kinematic compatibility and plate-area conservation.

## 4. Ancient continental foundations — all 13 cratons

Cratons are mechanically persistent lithosphere with old crust and thick, relatively depleted mantle roots. Their shield is the **exposed part** of a craton; the platform is the **sediment-covered part** of the same basement. The mobile belt is a potentially reworked crustal margin, not an automatically active present fault.

| Craton | Current plate | Basement (Ga) | Core area (Mkm²) | Shield (%) | LAB (km) |
| --- | --- | --- | --- | --- | --- |
| KAVRA | KORU | 3.20 | 5.59 | 18.9 | 206 |
| ORVEL | LYRAN | 3.34 | 7.37 | 53.4 | 211 |
| MAREN | SEREVAN | 2.85 | 7.99 | 23.7 | 195 |
| ILTHEN | BRINEVEIL | 2.74 | 4.03 | 21.8 | 191 |
| KEVAR | SEREVAN | 3.05 | 4.00 | 39.1 | 201 |
| SYRIN | MERROW | 3.45 | 11.82 | 39.6 | 214 |
| EIDEN | MERROW | 2.94 | 5.88 | 11.0 | 198 |
| NALOR | THALASS | 2.58 | 2.25 | 58.9 | 186 |
| TAVEN | ORREN | 3.18 | 2.42 | 20.2 | 205 |
| SELDAR | HALDEN | 2.82 | 4.95 | 40.2 | 194 |
| HERIN | VALLIN | 3.38 | 5.10 | 10.6 | 212 |
| VORUN | VALLIN | 2.69 | 1.26 | 7.5 | 189 |
| FARON | THALASS | 3.08 | 2.53 | 49.9 | 202 |

**Syrin–Eiden pair (Merrow):** an especially important cratonic nucleus in this world. Their older basement and shared plate make a long-lived stable center plausible. Their relative position does not by itself prove they were sutured in a specific era; a paleo-mobile belt must be dated before a precise collision can be inferred.

**Maren–Kevar pair (Serevan):** a broad cratonic platform plus a somewhat older shield could produce extensive older cover sedimentary basins and localized mineralization at platform unconformities. The proposed original shield-exposure classification does not imply that all of Kevar is exposed bare rock.

**Herin–Vorun pair (Vallin):** two older-to-younger blocks with distinct basement histories suggest an ancient suture or possible rift-fragment affiliation. The difference in age alone cannot determine whether they collided or shared a common igneous province.

**Nalor–Faron pair (Thalass):** isolated craton cores within a mixed ocean–land plate could be continental fragments that survived breakup. A future model should ask whether their 2.58/3.08 Ga basement records separate accretion or different components of one ancient continent.

**Kavra, Orvel, Ilthen, Taven, Seldar:** more isolated cratonic domains. Their long-term resistance to deformation can deflect young thrust belts and influence the placement of large foreland basins, passive-margin shelves and lithospheric rifts.

Cratonic ages in this model are core basement ages, not necessarily crystallization times for *every* rock within a region. Multiple lithologies and episodes of metamorphism can coexist within a single craton.

## 5. Full lithospheric structure down to 300 km

The new material parameters are **prior assumptions** designed to be credible as starting ranges, not solved physical fields. Files contain the estimated crust thickness, depth to Moho, LAB depth and heat-flow atlas layers, plus reference columns down to 300 km.

| Province | Area (Mkm²) | Median crust (km) | Median LAB (km) | Heat (mW/m²) |
| --- | --- | --- | --- | --- |
| ocean basin | 268.1 | 7.0 | 84.0 | 47 |
| ocean spreading axis | 19.8 | 7.0 | 33.4 | 125 |
| ocean trench | 15.8 | 7.0 | 41.8 | 88 |
| noncratonic continent | 107.9 | 35.0 | 105.0 | 58 |
| exposed cratonic shield | 18.8 | 41.0 | 205.4 | 40 |
| sedimentary cratonic platform | 42.3 | 39.0 | 202.1 | 40 |
| ancient mobile belt | 13.2 | 37.0 | 125.0 | 53 |
| collision orogen | 14.2 | 43.5 | 118.0 | 65 |
| volcanic arc | 1.9 | 36.0 | 92.0 | 83 |
| continental rift | 5.4 | 24.0 | 67.0 | 89 |
| transform shear zone | 2.6 | 30.0 | 85.0 | 68 |

### 5.1 Crust vs lithosphere

The **Moho** is the crust–mantle boundary; the **LAB** is the relatively strong lithosphere–weaker asthenosphere transition. Both are physical boundaries, not surface elevations. A 40-km-thick cratonic crust can be supported by a 210-km-thick lithospheric plate. Under a collision, the crust can thicken without the entire lithosphere doubling in depth; mantle delamination and root loss are possible on a longer timescale.

The current scenario uses ~7 km for normal oceanic crust, 24–41 km for common modeled continental provinces, ~43 km median in the active collision class, and ~190–215 km lithosphere under most ancient cratonic cores. Some orogens in nature exceed 60 km crustal thickness; this generator's current smoothing and assumptions do **not** yet reproduce that full range at most collision sites.

### 5.2 Upper mantle and thermal profile

The craton root resists rapid stretching, making it a plausible anchor for long-lived plate motion and a source of large contrasts in gravity, geoid, seismic velocities and heat-flow patterns. The LAB depth is modeled as an age-dependent cool-root value at cratons and much shallower under rifts and some arcs. The oceanic lithosphere thickens with the square root of an *ocean-age proxy*, not a real seafloor age reconstruction.

The 0–300 km column profiles in the JSON file give illustrative temperature/rock-domain labels. They are intentionally simplistic: temperature interpolates from ~15°C at surface to ~1300°C at the modeled LAB and grades only weakly into asthenosphere below. Real models require conductivity, radiogenic heat production, mantle adiabat, transient heating, crustal generation, and plate thermal age. The column's temperature is **not a melt predictor** or an energy-conserving solution.

### 5.3 Isostasy and topography

The surface DEM is held fixed from our previous map. Therefore its mountain elevations do not necessarily balance the new crustal roots. A physically stronger iteration should solve Airy/Pratt-style isostatic reference states, mantle dynamic topography and erosion. In particular, a high mountain belt next to a rift requires compatible uplift/necking and enough lithosphere to sustain the load. Ocean ridges ought to show a broad thermal swell decaying with seafloor age, while trenches should connect to genuine subducting slabs and adjacent basins.

## 6. Ocean-floor construction, bathymetry and basin evolution

**Current ocean-age layer:** `ocean_age_proxy_ma`. It reproduces the *previous terrain code's age parameter*, approximately `min(200, distance_to_ridge_pixels × 0.88)`; this is not derived from actual plate transport across time. The existing bathymetry was generated with a pixel-distance subsidence approximation, and is retained for backward compatibility. It must not be treated as independent corroboration of plate motion.

For a stronger new seafloor model, every ocean-floor cell should store creation time, source ridge segment, parent plate, magnetic chron, later deformation, and distance from the ridge by integrating plate rotations. With a modeled thermal age, ridge axial depth around 2.5 km and a young seafloor subsidence rate around 320–350 metres times sqrt(age in Ma) provide useful first-order reference behaviors, before correcting for sediments, seamounts, thermal anomalies and dynamic topography. Older lithosphere normally departs from simple indefinite half-space cooling; a plate-cooling law is needed.

Ocean-floor **creation** at divergent contacts and **destruction** at convergent contacts must conserve area over geological stages (within explicitly chosen numerical tolerance) and must not simply create negative-height textures. No such time-integrated area or lithospheric mass budget exists in v1.0.

## 7. Proposed geological history (hypothesis, not solved reconstruction)

A specific ancient map cannot be inferred uniquely from the present landmask, 17 current Euler poles and 13 designed basement ages. There are multiple compatible or mutually contradictory paleogeographies. The following is one causal scenario designed to guide a simulation.

| interval | units | stage | tectonic_processes |
| --- | --- | --- | --- |
| 3450–3000 | Ma | Ancient nuclei form | Felsic proto-crust formation, greenstone belts, mantle-root stabilization |
| 3000–2500 | Ma | Late Archean crust growth | Growth and annealing of additional cratonic blocks; stabilization predates most younger sutures |
| 2500–1800 | Ma | Proto-continental amalgamation | Mobile belts accrete onto older nuclei; deformation and high-grade metamorphism |
| 1800–1000 | Ma | Cratonic recycling and platform development | Metamorphism and sedimentary platform development on stable interiors |
| 1000–650 | Ma | Long-lived ancient suturing | Inherited sutures and reworked continental margins; mobile belts |
| 650–300 | Ma | Basinal sedimentation and mature continental roots | Epeiric basins, passive-margin sediments and crustal shortening locally |
| 300–220 | Ma | Pre-breakup continental assembly | Assembly and weakening along inherited suture systems |
| 220–145 | Ma | Continental extension and breakup | Rift basins, flood basalt potential, new passive margins; resistant cratonic cores remain |
| 145–80 | Ma | Oceanic-basin expansion | New oceanic crust, cooling-related subsidence and marine transgression |
| 80–25 | Ma | Subduction and collision onset | Orogen growth, trench advance and arc volcanism |
| 25–0 | Ma | Modern 17-plate kinematic pattern | Present Euler pole motion, inferred boundaries, ongoing shortening and extension |

Every reconstruction stage must eventually get its own plates/rotations and birth–death topology, rather than applying the modern rate for billions of years. Present angular velocities only represent an instantaneous epoch. Their purpose is to initialize one active state, not retroactively generate any given ancient supercontinent.

## 8. Rock provinces, metamorphism, geochemistry, and mineral-system potential

This model separates the age of **basement**, age of **last deformation** (not yet solved), **cover sediment**, and crustal **tectonic setting**. A cratonic shield can contain much younger intrusions than its oldest zircon ages; likewise a recent mountain belt may expose ancient basement. Do not color a whole region as uniformly 3.45-Ga bedrock without distinguishing geological units.

- Ancient shield provinces: granitoid-greenstone terranes, banded iron formation, locally metamorphosed volcano-sedimentary sequences and intrusive complexes. Mining associations can include iron, orogenic gold and locally nickel-copper or kimberlite-related diamond potential—only if critical source, fluid and preservation conditions exist.
- Cratonic platforms: old basement covered by shallow-marine to continental sedimentary basins. Possible source rocks and reservoir sequences depend on subsidence, sediment flux, burial, temperature, water chemistry and preservation.
- Paleo-mobile belts: reworked margins and sutures, structurally plausible controls for old metamorphic and hydrothermal belts, not necessarily present-day seismic belts.
- Collisional mountain belts: thrust sheets, nappe stacks, crustal partial melting and high-grade metamorphic cores, with flysch/molasse and clastic foreland depocenters.
- Subduction-arc belts: trench sediments, accretionary wedges, mafic-to-felsic volcanic and plutonic rocks; hydrous melting and porphyry/epithermal mineral systems where chemistry and fluids fit.
- Continental rifts: brittle extension, fault-bounded half-grabens, basaltic volcanism and thermal maturation of source rocks subject to sedimentation history; possible active geothermal resources.
- Ocean spreading centers: new basaltic crust, hydrothermal vents and massive sulfide systems, ridge-flank normal faults, abyssal sediments farther away.

**Mineral association fields are qualitative play concepts, not deposit maps.** None represents ore volumes, grades, market quantities or resource probabilities. These require explicit geochemical, burial, fluid and erosion histories.

## 9. Seismicity, fault depth and mantle structure

The raster `deformation_tendency_0to1` is a normalized distance-to-boundary signal, not a global seismic-hazard forecast. It does not contain stress, strain accumulation, recurrence intervals, friction parameters, seismic catalogues or rupture statistics. Thus **0** means far from modeled active boundaries, not zero hazard; **1** means high modeled proximity, not an annual earthquake probability.

A full simulator should establish: plate coupling coefficients; stress transfer on major named fault segments; crustal brittle thickness; transform release/restraining-bend kinematics; thrust vergence; slab depth contours; geothermal rheology; viscoelastic lower crust; and explicit stochastic rupture models. Likewise, volcanic arcs cannot be precisely placed until subduction polarity and slab dip are known. An ocean-to-continent convergent line does not identify on its own which side goes down.

## 10. Implementation data dictionary

### Core native raster stack

The compressed NumPy NPZ is the canonical aligned data resource. Each grid field is `H×W` with 941 rows and 1,672 columns. The supported fields are:

`plate_id`, `craton_id`, `land_mask`, `shield_mask`, `paleomobile_mask`, `boundary_class`, `province_id`, `elevation_m`, `crust_thickness_km`, `lithosphere_thickness_km`, `heat_flow_mW_m2`, `basement_age_ma_known`, `ocean_age_proxy_ma`, `deformation_tendency_0to1`, and four `distance_*_km_approx` layers.

The seven GeoTIFFs include the most useful single layers for GIS, Gaea-style terrain systems, and simulation tools. Plate and craton polygon GeoJSONs are extracted from the native IDs; boundary LineStrings are traced from actual contact edges. Attributes are not Earth place names or modern GPS positions.

### Reproducibility

`build_dataset.py` regenerates all numerical results from original source assets and previous scripts, without using an external geographical tectonic database. `atlas_field_dictionary.json` defines every raster field and missing-data convention. `summary.json` includes checks and global metrics. `MANIFEST.sha256` verifies file bytes in a package.

## 11. Validation gates before using this as a dynamical world

**Gate A — topology:** Every plate region is valid and covers the full sphere, adjacent contact arcs are not duplicated, all triple junctions have permissible motions, and longitude-seam contacts close properly.

**Gate B — geological identity:** Old stable cratonic roots survive unless a defined rifting or collision stage destroys them. The shield/platform split is driven by denudation and sedimentation, not simply selected by artistic shape.

**Gate C — kinematics:** Stage Euler rotations connect without location jumps or angular-rate discontinuities; divergent spreading creates ocean crust while subduction destroys compatible-aged ocean crust; net planetary surface and crust mass are conserved under the chosen model.

**Gate D — dynamics:** Convergent shortening results in plausible crust thickening, thermally weakened lower crust, uplift and/or delamination; ocean-floor bathymetry is consistent with age and thermal cooling.

**Gate E — geomorphology:** Mountains and basins emerge from uplift and flexure, then undergo physically based weathering, sediment transport and erosion. Surface runoff drains to ocean or genuine internal basins rather than arbitrary isolated depressions.

**Gate F — external calibration:** Physical values are checked against ranges from terrestrial rock mechanics, heat flow, crustal thickness, and seafloor subsidence—but no assumption is made that this fictional world reproduces Earth's specific plate geometry.

## 12. Recommended engineering order

1. Freeze current map, plate IDs, craton masks and analytical Euler poles as v1.0; version their hashes.
2. Replace the proximity-derived crust types with persistent crustal tiles carrying age, density, composition, thickness and source block ID.
3. Introduce stage-specific plate topology and rotations, moving cratons as rigid polygons where appropriate and allowing rift fragments.
4. Resolve subduction polarity, trench/arc pairings, and spreading-isochron birth dates.
5. Solve crust/lithosphere thermal and isostatic structure, geological uplift, flexure, sediment production and erosion at a coarser global mesh.
6. Resample conservative physical fields into terrain-chunk/voxel world representations without making every rock voxel simulate tectonic motion.
7. Only after validating mass/area and energy checks should the synthetic relief/biomes/ores/water systems be regenerated.

## Physical reference anchors (used for parameter plausibility, not as map data)

- Pearson et al., *Deep continental roots and cratons* (Nature, 2021): https://www.nature.com/articles/s41586-021-03600-5 — old persistent lithospheric roots commonly of order 150–250 km.
- *Origin, Accretion, and Reworking of Continents* (Reviews of Geophysics, 2021): https://agupubs.onlinelibrary.wiley.com/doi/full/10.1029/2019RG000689 — broad contrasts in continental/oceanic crustal and lithospheric properties.
- Korenaga & Korenaga, *Subsidence of normal oceanic lithosphere* (Earth and Planetary Science Letters, 2008): https://www.sciencedirect.com/science/article/abs/pii/S0012821X08000125 — cooling, depth-age behavior and limitations.
- GPlates/pyGPlates reconstruction hierarchy: https://www.gplates.org/docs/pygplates/pygplates_primer — Euler pole and stage reconstruction conventions.

**Final status:** An executable, analyzable *initial-condition geologic model*. Not a validated geological record or a backward-integrated crust-formation history.
