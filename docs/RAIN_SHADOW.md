# Rain-shadow map — terrain-only proxy v1

## Purpose

Rain Shadow is a **dedicated physical-world map**, separate from annual/monthly precipitation and Köppen classification.

From a land elevation DEM and a selected *incoming* prevailing wind direction, calculate a **signed orographic exposure index**. It is viewable in both the 2D atlas and the orthographic globe, spatially inspectable, and exportable as a full-resolution floating-point TIFF with JSON metadata.

This is a plausible physical mapping *heuristic*, **not** a rain gauge, dynamic atmospheric model, or calculated precipitation in mm/year. More realistic rainfall requires a moisture field, variable atmospheric circulation, cloud/condensation model, and seasonal parameters.

## Inputs

- Native-resolution planetary elevation raster (metres; negative = ocean).
- Wind arriving **from** one of eight compass directions; defaults to **West**, meaning flow eastward.
- Assumed 6,371 km spherical radius and uniform horizontal wind for P0. Planet-specific radii and spatially varying wind grids are future enhancements.

The tool is generic: it does not depend on Aegaea plate/craton labels or continent shapes.

## Output

`rain_shadow_orographic_exposure_index`:

| Index | Interpretation | Suggested map color |
|---|---|---|
| -1 … -0.1 | Orographic uplift on upwind slopes; *potential* enhancement | Cool blue/teal |
| -0.1 … +0.1 | Neutral or weak topographic response | Neutral sandstone/slate |
| +0.1 … +1 | Behind an upwind barrier; *potential* leeward drying | Amber/red |
| NaN | Ocean or missing DEM | Deep blue |

Every computed cell remains an independent numeric datum. The same signed index is sampled for the 2D atlas, 3D globe preview and inspector, and is not baked into an image.

### Algorithm (v1)

For each land cell, trace a great-circle-*approximated* upwind ray out to 700 km in 35 km steps on the native grid. The highest upstream relief above the current location indicates shelter; its influence decays with distance. Rising local terrain relative to the near-upwind ground indicates possible windward uplift. Stop the search at ocean to avoid carrying a barrier effect across an intervening marine moisture source. The resulting signed index is capped at [-1,1].

This is **not** a rigorous moisture-transport integration, and the uniform incoming wind deliberately ignores latitude bands, storm tracks, and seasonality. It should not be used to declare a region arid or humid without broader climate data.

## User workflow

1. Restore or load an elevation DEM; Aegaea's original DEM may be stored as `.binary-parts`, read by Studio without destructive restoration.
2. Select **Rain shadow** in the thematic layer list.
3. Choose **Wind from** West, Northwest, etc.; output recomputes without modifying the source.
4. Inspect a mountain's windward slope and downstream region on either map.
5. Export the numeric TIFF + sidecar JSON; the wind-direction assumption is included in metadata.
6. Save the project. Wind direction is preserved in the JSON project sidecar and re-evaluated on reopening.

## Tests and known limitations

- West-origin winds put the lee east of a synthetic ridge; reversing winds reverses the index.
- Ocean pixels remain NoData; output has source-grid dimensions; deterministic.
- Future: actual wind vector fields, seasonal/annual map series, moisture transport, precipitation coupling, manual layer corrections, exposed wind arrows, coastline ocean recharge, polar geodesic tracing, quantitative validation.
- This **does not** implement seasonal rainfall values or a validated climate simulation.

### Scientific basis

- [World Meteorological Organization: Orographic influences](https://cloudatlas.wmo.int/en/orographic-influences-on-the-windward-side.html) — sufficient humidity, rising windward air and descending leeward air govern the phenomenon.
- [American Meteorological Society: Rain-shadow glossary](https://glossary.ametsoc.org/wiki/rain-shadow/) — rain shadow is reduced precipitation leeward of an orographic barrier, *relative* to the windward side.
