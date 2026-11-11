# Design references: Gleba, Gaea 2, and World Machine

**Purpose:** Derive implementation principles, not reproduce proprietary code, user interfaces, artwork, node names, or algorithms. Aegaea remains a **planetary physical-world cartography workbench**, not a game simulator or a clone of any one tool.

Sources consulted:
- [Gleba by Calandiel](https://calandiel.itch.io/gleba)
- [Gleba: Winds, ocean currents, and orbs](https://calandiel.itch.io/gleba/devlog/1452492/devlog-3-winds-ocean-currents-and-orbs)
- [Gleba: Going interplanetary](https://calandiel.itch.io/gleba/devlog/1567515/devlog-4-going-interplanetary-v030)
- [Gaea 2 graph workflow](https://docs.gaea.app/ui/interface/graph/procedural-workflow.html)
- [Gaea node families](https://docs.gaea.app/reference/nodes/)
- [World Machine overview](https://www.world-machine.com/features.php)

## What to take from each

| Reference | Applicable principle | Aegaea implementation direction |
| --- | --- | --- |
| Gleba | Planetary fields must be causally consistent: crust, plate kinematics, topography, winds, ocean currents, climate, water, geology | Typed natural-science fields with explicit dependencies and time; inspectable raster and vector attributes |
| Gleba | Custom crust and plate imports constrain the result; imported heightfields alone do not make corresponding tectonics consistent | Preserve independent input datasets, show consistency warnings, never invent linked plate topology |
| Gleba | Monthly sampling and distinct map modes expose seasonal winds/rainfall | Time-axis field type and monthly layers in climate phase, with explicit scientific confidence |
| Gaea | Non-destructive node chains and reusable recipes | Deterministic acyclic typed graph, inspectable per-node outputs, eventually dockable advanced visual graph |
| Gaea | Generators, transforms, derived masks and output nodes have separate roles | Distinguish source, modifying, derived analysis, simulation, classification and export operations |
| Gaea | Low-cost previews and reusable baked nodes | Cached quick preview and explicit final build, retain seeds/parameters and dependent dirty flags |
| World Machine | Modular devices, macro grouping, procedural edit history | Named reusable recipes, grouped operations, parameterized macros after real workflow proves need |
| World Machine | Tiled/regional builds and portable outputs | Streaming and source-native tile caches, edge/halo-aware calculations, numeric GeoTIFF/PNG/SVG exports |

## Critical architecture choice

**Keep one canonical world dataset, with two distinct control surfaces:**

1. **Simple map editor**: pick a layer, paint a feature, generate a map, adjust the few relevant parameters, inspect a location. This is the primary UI.
2. **Advanced process graph**: optionally inspect/reconnect upstream sources, processes and derivatives, make branching outputs, bake/cache expensive steps, audit exactly how the map was made.

Do **not** make all geography into an untyped heightmap, because an ID-coded plate/craton map, floating precipitation field, month-by-month wind vectors, and geometry topology are scientifically different data. First graph iteration restricts itself deliberately to numeric **elevation metres**, **surface slope degrees** and **terrain-only orographic index**, with enforceable units. Later typed ports will include categorical fields, vector velocity, geology, and time series.

## Order of operations

1. **Now:** typed graph runner, source vs authored/derived identity, unit checks and reproducible derived maps with numeric export.
2. **Next:** map library quick-run presets; preview/result selector and advanced dockable node diagram. Do not expose an empty decorative graph pane before it does useful work.
3. **Then:** dirty-region invalidation, multiresolution/tiled builds, memory budget and immutable cached outputs; test seams, poles and physical unit consistency.
4. **Then:** terrain-specific operations (physical slope, curvature, flow, masks, modifiers); provenance for each operation and manual override preservation.
5. **Later:** geological reconstruction, conservative hydrology/erosion, ocean and atmospheric circulation; physically meaningful test gates before climate and ecology map families.

## Project constraints and known limitations

Aegaea has an existing fictional native raster orientation: **top south, bottom north; left +180°, right −180°**. All processing must preserve it. Native v1 is 1,672×941 with 17 plates and 13 cratons. Earlier planet reconstructions were scenarios rather than solved plate histories.

The current terrain edit mechanism recomputes dependent maps synchronously at stroke completion; Gaea/World Machine-style tiled caching would address this. The current rain-shadow operator is a signed terrain-only heuristic, **not precipitation simulation**. Globe and map data should retain the same scientifically typed inputs.

**Out of scope:** voxel terrain, human geography, polities, campaigns, NPCs, economics, full galaxy simulation and copying any proprietary generator.
