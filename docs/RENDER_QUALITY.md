# Render quality defects observed in the user's November 10 recording

**Source:** \`Recording 2026-11-10 221111.mp4\`, 149.4 s, 1916×1018 screen capture. This is a real Windows app run, not an illustrative mockup.

## Observations

| Approx. time | Visible behavior | Root-cause assessment |
| --- | --- | --- |
| 0–30 s | 2D physical atlas, tectonic overlays and categorical maps occupy an upper rectangle with a largely unused work area | Current canvas sizes itself to the source image and subtracts 30 px from the image height; no fill-crop option; aspect can be slightly distorted |
| 60–75 s | Zoomed terrain is mottled, repetitive oval mountain belts and hard-edged coastline outlines | Source DEM contains synthetic low-frequency morphology; one-sided per-pixel 1000 m contour code adds aliased stipples; shoreline stroke too heavy |
| 75–95 s | Globe looks dark, with strong triangular dark spikes converging near a pole | Globe mesh was **inward wound** with no back-face culling/depth; a fragment-only visible-z check allowed overlapping projected polygons; latitude/longitude converge at each pole but their source raster elevations/colors do not |
| 95–149 s | Dark categorical/geophysical globe layers are difficult to distinguish | The atlas texture is already shaded; an additional globe-specific lighting multiplier suppresses the whole image |

### Correctness fixes in this checkpoint

1. Reverse mesh triangle indices to *outward-facing* winding. Turn on GPU back-face culling instead of drawing both hemispheres through the same paint callback. Add a geometric test for the outward normal.
2. A globe has one pole, not 720 geographically distinct points. Average geometry elevation at each pole, and harmonize only the globe *presentation texture* within a narrow polar band. Preserve the actual scientific rasters and source coordinate convention.
3. Reduce the extra globe shading, which made geophysical maps nearly black in the capture.
4. Remove the one-neighbor coarse elevation contour test. Those were not true isolines and caused one-sided stair-stepped speckles. Lighten coastline ink.
5. Add **Fill workspace** (aspect-preserving UV crop) and **Fit entire planet** modes, scroll zoom and proportional pan. Never stretch geography to fill a rect.

### Acceptance

- Cargo tests, WGSL validation and exported map artifacts pass on Windows.
- Outward mesh normal for a visible equatorial triangle, both geographic poles share a single elevation, globe-only texture has a constant color at each pole.
- Atlas fill mode crops source UVs without changing the source aspect ratio; whole-map mode shows the complete source; map panning remains in bounds.
- **Manual desktop QA is still required**: rotate the poles, inspect two categorical maps for sufficient brightness, inspect coastline at 2–3x, confirm no warped map proportions and correct marker locations. A GPU shader compilation test **cannot** verify driver runtime or artistic quality.

### Explicitly still inadequate

The geological reference source is a **synthetic heuristic terrain snapshot**, not a solved tectonics/erosion model. Its overly smooth interiors, oval/radial mountain structures and other unnatural surface features cannot be repaired by raster colors or shading. Improving the generation process must be its own scientifically qualified, reversible phase. This quality gate makes the present maps less wrong, not Gaea/World Machine quality.
