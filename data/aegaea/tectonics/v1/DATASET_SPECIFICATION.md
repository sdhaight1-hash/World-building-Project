# AEGAEA — fictional tectonics dataset v1.0

## Scope

The dataset is *derived from the exact raster and source generator files* of the previously
constructed upside-down/horizontally-mirrored Aegaea world. It contains 17 instantaneous
Euler-rotation plates and 13 separate, ancient cratonic basement cores.

**It does not claim to be the output of a dynamically valid multi-billion-year tectonic model.**
The values for thermal structure, crustal thickness, tectonic provinces, and paleogeographic
chronology are explicit scenario assumptions with usable parameter values.

## Coordinate convention

Native raster dimensions: 1672 columns x 941 rows. This is the native computational grid.
The 8K rendered image has interpolated pixels, not independent geological observations.

- At screen-left, longitude is +180 degrees, decreasing to -180 on screen-right.
- At screen-top, latitude is -90 degrees; it increases to +90 at screen-bottom.
- Hence screen-right means geographic WEST, and screen-down means NORTH.
- Coordinate formulas at grid vertices: `lon = 180 - 360*x/1672`;
  `lat = -90 + 180*y/941`. Pixel-center coordinates add 0.5 to x/y.
- The longitude axis is periodic. The actual coordinates deliberately DO NOT represent
  Earth locations; do not overlay with Earth EPSG:4326 geospatial products.
- The assumed planetary mean radius is 6371.0 km; this sets kinematic and area units.

## Contents

- `summary.json` — quantitative global metrics, validation results, cautions.
- `plates.csv` — exact Euler poles/rates with calculated plate areas and speeds.
- `cratons.csv` — all 13 source cores, basin/shield split, age, area and LAB estimates.
- `plate_contacts_by_type.csv` — all adjacent plate pairs per active contact class,
   lengths, normal velocities, convergence/spreading/strike slip, broad products.
- `crustal_provinces.csv` — mapped crust/thermal provinces; numeric IDs in field dictionary.
- `tectonic_history_scenario.csv` — proposed ancient-to-modern chronology, NOT a solved reconstruction.
- `mineral_system_hypotheses.csv` — possible depositional environments, NOT resource locations.
- `plate_polygons.geojson`, `craton_polygons.geojson` — polygon geometry tied to raster IDs.
- `boundary_segments.geojson` — traced local classified boundary line segments, from actual label contacts.
- `reference_vertical_columns.json` — plate/craton reference columns to 300 km depth, heuristic geotherms.
- `aegaea_geology_native_1672x941.npz` — all 18 aligned raster layers (see `atlas_field_dictionary.json`).
- `plate_id.tif`, `craton_id.tif`, `elevation_m.tif`, `crust_km.tif`,
  `lithosphere_km.tif`, `province_id.tif`, `heatflow_mWm2.tif` —
  lossless GeoTIFF data; deliberately omit Earth CRS due to mirrored fictional axis.
- `build_dataset.py` — executable source code, uses earlier source files from /mnt/data.

## Physics and methods

### Plates, motion and contacts

Each of the 17 plates is assigned an Euler pole (lon,lat) and signed angular
rotation (deg/Ma) as defined in `make_fictional_tectonics.py`. The local rigid
velocity is `v = omega x radius_vector`; radius=6371 km. Relative velocities on
neighboring raster cells determine normal motion and boundary type. Transform
class where `|normal| < max(0.9, 0.28*|relative velocity|)` cm/year; positive
normal is extension. Negative normal is convergence, further designated
collision in mostly continental local neighborhoods, otherwise subduction.
Classification is **heuristic**, especially at narrow continental margins and
where one plate is fully oceanic. No subducting-plate polarity or slab geometry
has yet been solved.

The output summaries report one global class per local cell contact, but individual
plate pairs can have different boundary classes along their length. Their total
pixel boundary length uses spherical line metrics, so contact-length measurements
are discretization-dependent estimates, not surveyed fault-length measurements.

### Cratons

Source craton IDs/ages/exposure are retained exactly from the original craton
mask and metadata. The shield mask is a visible-exposure subset of a larger
craton, not a second separate plate. The adjacent paleo-mobile mask is a
schematic possible inherited suture belt, not evidence of a precise ancient fault.
The original generator clipped cores at present plate limits, suppressing
rifted matching craton fragments. Ancient affinity between separated cores
requires dedicated future reconstruction.

### Crust and lithosphere (additional assumptions)

Ocean crust = 7 km. Noncratonic continental crust = 35 km. Stable cratonic crust
= 39–41 km. Collisional orogens may thicken locally to ~42–50 km in this version;
rifts thin to ~24 km. Strong old cratonic mantle roots = ~186–214 km, and can
locally reach about 214 km according to age. Other continent ~105 km; active
rift ~67 km; thermal ocean lithosphere grows roughly with sqrt(age proxy).
The heat-flow defaults (~40 mW/m² cratonic, 58 continental, up to ~125 ridge)
are stylized reference values, not an energy-conserving thermal calculation.
The vertical columns use a schematic 15 C at the surface grading to 1300 C at
LAB, not geothermal PDE output. Do NOT use those values as predictive stress,
melting or geochemistry without additional calibration.

### Age and bathymetry

The existing DEM elevation is reused byte-for-byte; it is not refitted to
new crust/lithosphere parameters. Ocean age is deliberately named
`ocean_age_proxy_ma`, because it comes from pixel distance to spreading ridges
and a fixed coefficient (0.88 Ma/pixel) used by the previous terrain generator.
It is NOT a kinematic, magnetostratigraphic or paleogeographic age solution.
Example real-world half-space-cooling reference: approximate depth(m) =
2500+350*sqrt(age_Ma) only over young oceanic age ranges; it is not implemented
as a hard constraint here.

## Validation and known limitations

- Tests: all 17 source plate IDs present, all 13 cratons present and on land,
  craton masks assigned to their original single plate, global area sums to
  4*pi*R^2, and DEM unchanged.
- Not validated: stress balance, force balance, trench polarity, slab dip/depth,
  dynamic topography, oceanic crust production/conservation, erosion,
  isostatic compensation, terrain coherence with modeled Moho/LAB, or
  sediment transport.
- The terrain was generated *before* cratonic thermal/mechanical properties.
  Any physically-coupled future terrain regeneration must take those roots into account.
- Stage history is an editable hypothesis; you cannot run the present Euler pole
  model backward 3.5 billion years and expect a physically meaningful craton history.
- Mineral-system associations are broad geological possibilities, never
  verified mineral deposits or probabilities.

## Quick Python use

```python
import numpy as np
z = np.load('aegaea_geology_native_1672x941.npz')
print(z.files)
print(z['plate_id'].shape, z['craton_id'].max())
# A single location: (row 500, column 600)
print(z['elevation_m'][500,600], z['lithosphere_thickness_km'][500,600])
```

## Recommended next physically consistent step

Use this present-day model as boundary/initial conditions. Create explicitly
bounded age-stage rotations and plate topology changes, pair old cratons by
paleosuture evidence, solve subduction *polarity* and dynamic uplift, then
run an erosion/sedimentation and isostatic response model. Without that,
this model is an internally reproducible procedural hypothesis, not an
empirically established geologic chronology.

## Reproduce this build from the archive

The ZIP contains all eight input files under `sources/`, including the native
source map, Euler-pole/craton scripts, basement masks, and native source DEM.
After extracting, run in the `Aegaea_Deep_Tectonics_v1` directory:

```bash
python -m pip install -r requirements.txt
python build_dataset.py
```

The generator looks for `sources/` beside itself and writes new output files into
its own directory. No network calls are required. Requires Python 3.10 or later.
The original inputs remain untouched.
