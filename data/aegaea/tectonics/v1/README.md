# Aegaea Tectonics v1

Original synthetic geology and tectonics for an inverted, horizontally mirrored fictional planet.

This import contains 17 plate Euler models, 13 craton records, classified plate-contact tables, crustal provinces, mineral-system hypotheses, an illustrative geologic chronology, and source modeling documentation. All values are fictional and many subsurface quantities are initial-condition scenarios.

**Important:** The full local science archive also includes raster/NPZ/GeoTIFF and graphical assets that are not present in this initial text-only import. Consult `DATASET_SPECIFICATION.md` for the full archive manifest. Do not assume missing rasters are available in this repository.

The coordinates are custom fictional atlas angles, not real Earth geographic coordinates. Native analysis grid: 1672 × 941; 8K images are interpolated presentation output.

Start with `plates.csv`, `cratons.csv`, `plate_contacts_by_type.csv`, and `GEOLOGICAL_SYNTHESIS.md`. The source `build_dataset.py` requires its inputs under `sources/` before it can be run.
