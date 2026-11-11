# Source geology consistency — v1

Scientific-looking cartography requires **data agreement**, not merely several aligned images. Gleba's author highlights a concrete failure mode: importing a custom heightmap without matching plate and crust inputs makes climate use the heightmap but does not reconcile the underlying tectonics. Aegaea must avoid that hidden inconsistency.

## Current implementation

Read-only checks against loaded categorical rasters and source CSV metadata:

- Each **plate ID pixel** is an exact integer represented in the plate table.
- Each nonzero **craton ID pixel** belongs to a listed craton record; zero is background.
- Each **province ID pixel** has a corresponding geological class record.
- Each craton record's named **parent plate** exists in the plate table.
- For non-background craton pixels, the plate raster ID agrees with the craton record's assigned parent plate; report deviations.

The report includes raw cell counts, warning/error severity and explanatory descriptions. Pixel counts on a geographic equirectangular grid are **not area-weighted**. A mismatch does not prove the geological scenario is impossible: inputs may describe competing interpretations, unfinished edits or intentional fiction.

## Using it

From the desktop map sidebar choose **Geology consistency → Validate source layers**, then inspect findings and the generated \`geology-consistency.json\` next to the project workspace.

Headlessly from repo root:

\`\`\`powershell
cargo run -- --diagnose-world maps/diagnostics/geology-consistency.json
\`\`\`

Aegaea's actual source categories are checked in Windows CI and the report is published as an Actions artifact. No source TIFF, CSV or GeoJSON files are written or repaired.

## Explicit boundaries

This validator **does not** prove plate kinematic correctness, reconstruct tectonic history, independently validate climate, correlate coastline against crust, or reconcile a manually authored DEM with generated mountain morphology. It only validates category identity and directly expressed craton-to-plate relationships. Those scientific models require additional, separate tests and explicit uncertainty tracking.

Empty projects without tectonic rasters are valid and produce an informational message rather than an error.
