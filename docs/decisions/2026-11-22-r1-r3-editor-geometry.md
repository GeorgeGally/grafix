# R1 / R3 / early R4 — Authoring workspace and executable primitives

**Implemented:** 22 November 2026  
**Branch:** `feat/typed-graph-editor` (PR #2)  
**Design reference:** [Notch functional and UX audit](../plans/2026-11-21-notch-functional-ux-audit.md)

## User-facing functionality

- Resizable viewport/graph split and independently resizable graph palette/inspector columns.
- Quick-add with Tab, Cmd/Ctrl+K and an Add button; keyboard search / Enter.
- Graph minimap, graph arrangement by dependency, node duplicate/copy/paste shortcuts, undo support.
- Editable node titles, persisted as optional `label` in the backward-compatible v1 document.
- Inspector property filtering, precise numeric fields, sliders, per-property reset and grouped controls.
- Grid, Radial and existing Fibonacci Cloner options. Each has a real compiled distribution; native and custom GPU instances receive the same deterministic layout.
- Sphere (UV mesh with segment/ring counts), Cylinder, Plane and Torus with editable actual mesh dimensions, visibility, transforms and connected PBR materials.
- Non-cube geometry can feed the cloner: the selected shape/dimensions determine the instance mesh; geometry changes replace the shared source mesh only as needed.
- Scene node now owns `show_grid`, `ambient_color` and `ambient_brightness`. A blank authored scene has no visible demo floor. The bundled default demo explicitly requests its decorative grid.
- One LFO output can animate the amount of Bloom, Chromatic Aberration and Vignette simultaneously. Values are normalized and mapped to each effect's validated range. Wrong effect types on Output inputs fail validation.

## Architectural changes

The compiler projects `SceneItemDesc.geometry` and `Controls.clone_source_kind/clone_source_geometry` into Bevy. Scene objects own independent mesh handles; edits update the existing entity instead of replacing the whole object. Replaced/deleted geometry mesh and material handles are cleaned up.

Cloner layouts are generated in one shared algorithm before either native ECS instancing or GPU buffer upload, including seeded stable phases. Their group transform still triggers distribution re-preparation; buffer dirty tracking remains future work.

The editor continues to own a single graph document with Rust validation. Renaming keeps the opaque IDs stable, so existing connections do not break.

## Acceptance tests

1. Load the existing composition; create a Sphere from Quick Add, connect it to Scene and adjust radius and UV resolution independently from the Cube.
2. Create a material, connect it to the Sphere, and confirm metallic/roughness/color only affect the Sphere.
3. Connect that Sphere to a Grid Cloner; check all instances use the sphere mesh. Switch to Radial Cloner and compare shapes/modes.
4. Resize both workspace dividers. Duplicate, rename and rearrange nodes, save/reload and confirm coordinates/labels survive.
5. Select Scene: toggle the grid and change ambient color/brightness. Disconnect all objects to verify no demo-only geometry appears.
6. Feed one LFO into Bloom, Chromatic Aberration and Vignette; confirm all three amounts animate without rewriting authored base settings.
7. Test in actual Mac Chrome/WebGPU. Existing Chromium CI tests use a **mock WASM bridge**, so they do not prove GPU pixel changes.

## Outstanding from the broader plan

**This is not a full Notch-style renderer.** R2 full asset import/browser with persistent GLB resources, 3D picking/gizmos and separate editor camera, R3 deformer/effectors/multi-cloner and granular GPU dirty tracking, R4 shared transport/keyframe/timeline and deterministic seek, plus real-WASM GPU screenshot CI, remain unimplemented. Do not add UI controls for these until the execution contracts exist.

The first priority after this slice is **real viewport picking and transform gizmos**, paired with a real-WASM browser render smoke test; these close the largest remaining usability and correctness gaps.
