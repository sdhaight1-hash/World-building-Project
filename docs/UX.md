# UI/UX system — map first

## Visual goals

A modern, calm, professional desktop interface with a large map canvas, dark-first palette, accessible light option, restrained accent, clear typography and icons, tight spacing and good information density at **1920×1080**. No oversized cards or decorative dashboards in the workspace.

The initial UI shell establishes a narrow sidebar, top toolstrip, main atlas/globe viewport, inspector and low-profile status line. This is not a final UX implementation or proof of usability.

## Interaction contract

- Toggle 2D/3D with the same layer, selection, and coordinates retained.
- Treat layer display mode, color rules and opacity independently from field values.
- Contextual inspectors for units, provenance and source links.
- Drag and wheel to navigate; hover to query; click to select, with visible cursor/pick feedback.
- Point, line, polygon, raster brush and profile tools share toolbar placement and action verbs.
- Every destructive action can be undone; every generator previews its output before replacing a layer.
- Keep controls relevant to the current map type. 'Advanced' reveals numerical parameters without hiding them permanently.
- A meaningful empty state for blank projects; no broken UI when source fields are absent.
- Fast keyboard access to search, undo/redo, map switching, save and layer visibility.
- Exports should use templates with editable title, legend, scale bar, projection, sources and metadata.

## Accessibility and reliability

Keyboard navigation, readable labels, adequate contrast, scalable text, color-blind-friendly palettes and explicit categorical labels (do not encode data only by color). Persist layout, crash recovery and non-destructive editing are required before a production release.

## First acceptance session

At 1080p: open Aegaea -> choose a layer -> orbit globe -> click a craton and read its actual name -> switch to atlas -> mark an interesting location -> save -> reopen -> verify marker/coordinates and that original geology data is unchanged. Also open a new blank project without Aegaea files. Do not call the first UI complete until this is exercised on Windows.
