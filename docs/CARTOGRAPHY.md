# Data-driven cartography and map rebuild (P0.2)

The old renderer used basic color ramps, palette offsets unrelated to geological meaning, 1024-pixel-wide atlas previews and a 512-pixel globe. It also hid the reference world's geological boundaries in ordinary map views.

## What is now computed, not painted

| Map | Source data | Derived visual representation |
| --- | --- | --- |
| Physical relief/bathymetry | `elevation_m.tif` | Native-resolution hypsometry, bathymetric colors, DEM hillshading, coastline and 1-km land contours |
| Tectonic plates | `plate_id.tif`, `plates.csv`, boundary GeoJSON | Named plate fills, source color values, class-coded vector boundaries |
| Plate boundary classes | `boundary_segments.geojson`, terrain DEM | Subduction / collision / divergence / transform contacts with distinct colors |
| Cratons | `craton_id.tif`, `cratons.csv` | Named continental domains and their boundaries |
| Basement age | `craton_id.tif`, `cratons.csv` | Age gradient only where the source specifies a craton age |
| Crustal provinces | `province_id.tif`, `crustal_provinces.csv` | 11 named geological classes, not palette hash by ID |
| Crust thickness | `crust_km.tif` | Unit-aware continuous thickness ramp and relief |
| Lithosphere thickness | `lithosphere_km.tif` | Unit-aware continuous depth ramp and relief |
| Surface heat flow | `heatflow_mWm2.tif` | Unit-aware mW/m² ramp and relief |
| Rain shadow | derived from DEM + selected incoming wind | Terrain-only signed index, not simulated precipitation |

These map views share `Cartography::sample` rather than maintaining separate author-painted PNGs. Colors use the actual source fields (including plate color RGB and the four contact classifications). Coastline and terrain lighting are independently derived from the native DEM. Source files remain unmodified.

## Export usage

From repository root (Windows PowerShell):

```powershell
cargo run --release -- --export-maps maps/aegaea
```

Output includes one `.png` at source-native resolution for every available category, and a same-named `.json` sidecar containing map name, units, source coordinate convention, legend entries, and provenance qualification.

Use **Export map PNG** in the desktop toolbar to export the active 2D map and its sidecar to the workspace directory.

These PNGs are visually styled atlas exports, not georeferenced raster datasets; the original TIFFs provide the numeric source data. In the next map-composition milestone, add *print layouts* with embedded legends, scales, grids, and labels. Projection switching, true 3D terrain, multi-layer blending, and geological history reconstruction remain future functionality.

## Quality gates

- Source aligned: 1672×941 Aegaea data renders at 1672×941, no 1024-wide resampling.
- All 17 plate colors and all 11 province labels come from source metadata.
- Classified geological boundaries must load from over 4,500 line features and show all four contact classes.
- Cross-projection sampling must preserve the intentionally mirrored/inverted fictional coordinate mapping.
- Exported PNG is readable and matches the native grid; JSON sidecar holds the correct legend.
- Blank projects must render without assuming Aegaea's geometry, plate IDs or data files.
- Test coverage checks map output, round trip and source metadata on Windows CI.

## Limitations

The current system is a **native-resolution raster cartographic visualization**, not a physical world generator. It improves how existing geology is represented; it does **not** infer missing geological formations, simulate erosion, reconstruct plate history, or calculate new hydrology/climatology.
