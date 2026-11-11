# Aegaea Tectonics v1

A simulation-oriented, fictional geological atlas for the upside-down, horizontally mirrored Aegaea world. It contains **17 tectonic plates**, **13 cratons**, plate-boundary geometries, kinematics, geological provinces, modeled subsurface values and an illustrative geological chronology.

## Download, restore and verify

This repository contains all 22 original source/text/vector files and all **15 original binary assets**. Eleven binaries are ordinary Git files. Four larger binary files are preserved **byte-for-byte in ordered Git chunks** under `.binary-parts/`, to fit connector upload limits.

After cloning the repository, run from the repository root:

```bash
python data/aegaea/tectonics/v1/restore_assets.py
```

The standard-library script reconstructs the original NPZ, GeoTIFF and source NPY/image assets and validates all 15 binaries by SHA-256 and exact byte length using `ASSET_MANIFEST.json`. It will exit nonzero on any missing/corrupt part. To verify again later: `python data/aegaea/tectonics/v1/restore_assets.py --verify-only`.

Install generator dependencies if you wish to recompute derived data (this is *not* necessary to view or load the checked-in results):

```bash
python -m pip install -r data/aegaea/tectonics/v1/requirements.txt
python data/aegaea/tectonics/v1/build_dataset.py
```

The native numeric raster resolution is **1672 × 941**, not 8K. The 8K artwork is a visual upscale, not additional geological detail. Grid top is fictional south; horizontal axis points toward fictional west. These coordinates are **not Earth EPSG:4326** coordinates.

## Main artifacts

- `plates.csv` — original Euler poles/rates and plate metrics.
- `cratons.csv` — craton ages, plate associations and models of ancient lithosphere.
- `plate_contacts_by_type.csv` — boundary classification and relative kinematics.
- `boundary_segments.geojson` — 4,681 original synthetic boundary line segments.
- `plate_polygons.geojson`, `craton_polygons.geojson` — full spatial plate/craton extents.
- `aegaea_geology_native_1672x941.npz` — restored aligned geology arrays (18 layers).
- `elevation_m.tif`, `crust_km.tif`, `lithosphere_km.tif`, `heatflow_mWm2.tif`, `plate_id.tif`, `craton_id.tif`, `province_id.tif` — raster layers.
- `GEOLOGICAL_SYNTHESIS.md` — interpretive report, with known limitations.
- `DATASET_SPECIFICATION.md` — original comprehensive dataset README and coordinate conventions.
- `sources/` — original masks, map, synthetic heightfield and generator scripts.
- `tectonic_data_atlas.png` and `tectonic_boundaries_and_cratons.png` — derived visualizations (one is stored in chunks).

**Scientific caveat:** All plate velocities, inferred boundaries, craton ages, crustal properties and deep-time stages are fictional modeling assumptions or map-derived approximations, not validated plate-reconstruction dynamics or observational geology.
