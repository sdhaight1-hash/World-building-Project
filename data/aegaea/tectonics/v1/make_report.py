from pathlib import Path
import pandas as pd,json
P=Path('/mnt/data/aegaea_tectonics_v1')
s=json.loads((P/'summary.json').read_text())
pl=pd.read_csv(P/'plates.csv').fillna('—')
cr=pd.read_csv(P/'cratons.csv').fillna('—')
ct=pd.read_csv(P/'plate_contacts_by_type.csv')
pr=pd.read_csv(P/'crustal_provinces.csv')

def table(rows,cols):
 vals=[]
 for row in rows:
  vals.append('| '+' | '.join(str(row[c]) for c in cols)+' |')
 return '| '+' | '.join(cols)+' |\n'+'| '+' | '.join(['---']*len(cols))+' |\n'+'\n'.join(vals)

lines=[]
def add(t):lines.append(t.strip()+'\n')
add('''# AEGAEA — Comprehensive tectonic, crustal and cratonic synthesis

**Dataset version:** 1.0 (fictional atlas). **Native data grid:** 1,672 × 941 cells. **World:** fixed inverted/mirrored coastline. **Model type:** designed conditional tectonic scenario, not a coupled geodynamic or time-backward reconstruction.

## Executive geological interpretation

This fictional planet's present-day lithosphere consists of 17 instantaneous rigid Euler plates and 13 distinct ancient cratonic basement domains. Unlike copying Earth's known plates, the plate footprints are generated from an original warped seed system, each plate has a synthetic Euler pole, and each contact is classified from instantaneous relative kinematics. The ancient basement cores are clipped to their current plate and land masks. The original synthetic relief is preserved exactly. A second suite of *scenario* fields adds Moho depth, lithosphere–asthenosphere boundary (LAB), surface heat flow, and tectonic province classes.

The first two facts are numerical consequences of *our fictional source map*, not planetary observations. A crustal shield polygon, for example, is an assigned Archean core with implied durable lithosphere—not evidence that geological processes in the past really produced it. A cratonic age is a selected model age, not a radiometric measurement.

The planet radius is **6,371 km**, chosen only to establish sensible velocity, area and length units. The land-mask classification yields **'''+f'''{s['mapped_land_million_km2']:.2f} million km² land ({s['mapped_land_fraction']*100:.2f}% of its surface)'''+'''**. This number is conditional on image-color extraction: it is not a reliable replacement for a raster land/sea survey. It is larger than Earth's familiar 29%-land fraction partly because this is a reworked illustrative atlas.

The total assigned cratonic area is **'''+f'''{s['craton_core_land_area_million_km2']:.2f} million km² ({s['craton_share_land_fraction']*100:.1f}% of detected land)'''+'''**. These are *current exposed land* craton-core areas only; rifted offshore fragments or buried cores under continental margins have not been modeled. Their cratonic basement ages span 2.58–3.45 billion years.

## 1. Spatial reference and physical basis

The map is not a conventional north-up east-right terrestrial map. In atlas pixel coordinates, x increases to the right as longitude *decreases*, and y increases downward as latitude *increases*. Thus the top of the map is the south pole and its right side is geographic west. Longitude wraps at the left/right map seam. Convert pixel indices to the model's **fictional atlas** angular coordinates before ingesting into a globe shader or physics engine. GeoTIFF files intentionally have no EPSG CRS because attaching a real-world CRS would invite misleading geographic overlays. The 8K images are upscaled presentations; all numerically distinct data come from the source grid.

The 17 Euler rotations are expressed as pole longitude, pole latitude, and signed rotation angle per million years. Surface rigid-motion velocities satisfy `v = omega × r`. Relative normal motion between neighboring plate cells is divided into spreading, convergence, and transform motion. In a dominantly continental neighborhood convergence is labeled collision; otherwise it is labeled subduction. This simplified rule cannot identify slab polarity or trace oceanic microplate fragments, and may misclassify near shorelines and triple junctions.

The length of a plate contact is calculated by summing its north–south or east–west adjacency edges using the planet's spherical metric. It is *not* the length of a physically smoothed fault centerline. Values are therefore precise to the algorithm, not accurate geological measurement.

## 2. Inventory of all 17 modern plates

A positive Euler rate follows the input reference-frame convention and a negative rate reverses rotation. The maximum and weighted-mean plate speeds are in cm/year in the supplied arbitrary absolute reference frame, not toward/away from a neighbor. Plates can include both continental and oceanic crust.
''')
rows=[]
for _,r in pl.iterrows():
 rows.append({'Plate':r.plate,'Area (Mkm²)':f"{r.area_million_km2:.2f}",
 'Land (%)':f"{r.land_fraction*100:.1f}",
 'Pole (lon,lat)':f"({int(r.euler_pole_lon_deg)}, {int(r.euler_pole_lat_deg)})",
 'Rate (°/Ma)':f"{r.signed_rate_deg_per_Ma:+.2f}",
 'Mean speed (cm/yr)':f"{r.mean_abs_speed_cm_yr:.2f}",
 'Cratons':r.craton_names})
add(table(rows,['Plate','Area (Mkm²)','Land (%)','Pole (lon,lat)','Rate (°/Ma)','Mean speed (cm/yr)','Cratons']))
add('''**Interpretation:** Merrow, Lyran, Veyra and Halden contain extensive or medium-sized land portions and can sustain long-lived subduction-accretion or collision effects at their margins. Ankar, Caerin, Ruskel and Sundral are primarily oceanic within this source mask. Nivale is a partly land-bearing south-polar cap. These are present-day composition labels, not permanent identities; a craton can survive the eventual breakup of the plate currently transporting it.

## 3. Quantitative plate-boundary budget

The following lengths are the sum of individual classified contacts across the entire native grid. A *single plate pair may appear in more than one class* because contacts curve, pass onto land, or change local relative velocity along their trace.
''')
bud=[]
for kind,n in s['boundary_lengths_km_by_class'].items():
 sub=ct[ct.boundary_type==kind]
 bud.append({'Boundary':kind.replace('_',' '),'Length (km)':f"{n:,.0f}",'Share (%)':f"{100*n/s['total_boundary_length_km_est']:.1f}",'Contact records':len(sub)})
add(table(bud,['Boundary','Length (km)','Share (%)','Contact records']))
add(f"Total contact length approximately **{s['total_boundary_length_km_est']:,.0f} km** across {len(ct)} plate-pair–boundary-class records, represented by {s['boundary_linestring_features']:,} trace segments. These represent Euler-kinematic contact classes rather than fully strain-balanced fault zones.")

add('''### 3.1 Longest inherited model contact systems

**Convergent (putative subduction) systems** have a trench/forearc/arc/retroarc architecture only if oceanic lithosphere exists on the descending side, a polarity is chosen, and long-term convergence persists. Without polarity, the model can identify a possible zone, **not which side carries the volcanoes**.
''')
for kind,description in [
 ('subduction','Candidate trenches and volcanic arcs'),
 ('continental_collision','Candidate thick-crust orogens and foreland basins'),
 ('divergent_spreading','Candidate spreading axes and intracontinental rifts'),
 ('transform','Candidate long-lived strike-slip systems')]:
 rows=[]
 for _,r in ct[ct.boundary_type==kind].nlargest(8,'total_contact_length_km_est').iterrows():
  rows.append({'Contact':r.plate_a+' / '+r.plate_b,
               'Length (km)':f"{r.total_contact_length_km_est:,.0f}",
               'Rel. speed (cm/yr)':f"{r.mean_relative_speed_cm_yr:.2f}",
               'Normal rate (cm/yr)':f"{r.mean_normal_velocity_local_cm_yr:+.2f}"})
 add('#### '+description+'\n'+table(rows,['Contact','Length (km)','Rel. speed (cm/yr)','Normal rate (cm/yr)']))

add('''### 3.2 Physical interpretation of boundaries

**Subduction:** plate convergence can remove oceanic lithosphere and transport sediment and hydrated mafic material deep into the mantle. A complete model would solve slab polarity, dip and age, seismic coupling, accretionary-prism sediment flux, hydrated-wedge melting, and arc position ~100–300 km inland from the trench as geometry permits. The present raster only indicates convergence and whether the local land fraction is low.

**Continental collision:** strong crustal shortening leads to thickening, folding, crustal root development and partial melting; erosion redistributes that uplift into large foreland basins. A collision label is plausible only when *both sides* are buoyant continents. The present classification uses a blurred land cover proxy and therefore needs a future crust-type-on-each-side test.

**Spreading/rifting:** divergence creates oceanic crust at spreading centers or thins continental lithosphere until rupture. Mature spreading should be accompanied by age isochrons, a mid-ocean ridge heat-flow anomaly and roughly symmetric young crust on both flanks; all are future validation conditions. A divergent contact inside an ancient craton would require large rift forces or reactivation of older mobile belts, so its role is not accepted blindly.

**Transform:** lateral motion offsets crustal features and accumulates shear strain, with local uplift or pull-apart subsidence at restraining and releasing bends. Slow contacts (less than roughly 1 cm/year) can be effectively low-activity even if they are algorithmically marked transform.

**Triple junctions:** where three plates meet, contact geometry needs a stable topological solution. The simple source Voronoi plate generation does not enforce triple-junction stability; before deep-time evolution, junctions should be screened for kinematic compatibility and plate-area conservation.

## 4. Ancient continental foundations — all 13 cratons

Cratons are mechanically persistent lithosphere with old crust and thick, relatively depleted mantle roots. Their shield is the **exposed part** of a craton; the platform is the **sediment-covered part** of the same basement. The mobile belt is a potentially reworked crustal margin, not an automatically active present fault.
''')
rows=[]
for _,r in cr.iterrows():
 rows.append({'Craton':r['name'],'Current plate':r.plate,'Basement (Ga)':f"{r.basement_age_Ga:.2f}",
              'Core area (Mkm²)':f"{r.area_million_km2:.2f}",'Shield (%)':f"{r.shield_fraction*100:.1f}",
              'LAB (km)':f"{r.median_LAB_km:.0f}"})
add(table(rows,['Craton','Current plate','Basement (Ga)','Core area (Mkm²)','Shield (%)','LAB (km)']))
add('''**Syrin–Eiden pair (Merrow):** an especially important cratonic nucleus in this world. Their older basement and shared plate make a long-lived stable center plausible. Their relative position does not by itself prove they were sutured in a specific era; a paleo-mobile belt must be dated before a precise collision can be inferred.

**Maren–Kevar pair (Serevan):** a broad cratonic platform plus a somewhat older shield could produce extensive older cover sedimentary basins and localized mineralization at platform unconformities. The proposed original shield-exposure classification does not imply that all of Kevar is exposed bare rock.

**Herin–Vorun pair (Vallin):** two older-to-younger blocks with distinct basement histories suggest an ancient suture or possible rift-fragment affiliation. The difference in age alone cannot determine whether they collided or shared a common igneous province.

**Nalor–Faron pair (Thalass):** isolated craton cores within a mixed ocean–land plate could be continental fragments that survived breakup. A future model should ask whether their 2.58/3.08 Ga basement records separate accretion or different components of one ancient continent.

**Kavra, Orvel, Ilthen, Taven, Seldar:** more isolated cratonic domains. Their long-term resistance to deformation can deflect young thrust belts and influence the placement of large foreland basins, passive-margin shelves and lithospheric rifts.

Cratonic ages in this model are core basement ages, not necessarily crystallization times for *every* rock within a region. Multiple lithologies and episodes of metamorphism can coexist within a single craton.

## 5. Full lithospheric structure down to 300 km

The new material parameters are **prior assumptions** designed to be credible as starting ranges, not solved physical fields. Files contain the estimated crust thickness, depth to Moho, LAB depth and heat-flow atlas layers, plus reference columns down to 300 km.
''')
rows=[]
for _,r in pr.iterrows():
 rows.append({'Province':r.province.replace('_',' '),'Area (Mkm²)':f"{r.area_million_km2:.1f}",
 'Median crust (km)':f"{r.crust_thickness_median_km:.1f}",
 'Median LAB (km)':f"{r.LAB_depth_median_km:.1f}",
 'Heat (mW/m²)':f"{r.surface_heatflow_median_mW_m2:.0f}"})
add(table(rows,['Province','Area (Mkm²)','Median crust (km)','Median LAB (km)','Heat (mW/m²)']))

add('''### 5.1 Crust vs lithosphere

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
''')
add(table(pd.read_csv(P/'tectonic_history_scenario.csv').to_dict('records'), ['interval','units','stage','tectonic_processes']))
add('''Every reconstruction stage must eventually get its own plates/rotations and birth–death topology, rather than applying the modern rate for billions of years. Present angular velocities only represent an instantaneous epoch. Their purpose is to initialize one active state, not retroactively generate any given ancient supercontinent.

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
''')
(P/'GEOLOGICAL_SYNTHESIS.md').write_text('\n'.join(lines),encoding='utf8')
print('report words',len((P/'GEOLOGICAL_SYNTHESIS.md').read_text().split()))
