# Grafix R2–R4 implementation — authoring, imports, deformation and time

**Date:** 23 November 2026  
**Branch:** `feat/typed-graph-editor`  
**Scope:** The vertical slices implemented from the [Notch-inspired functional/UX audit](../plans/2026-11-21-notch-functional-ux-audit.md). This is **not** full R2–R4 completion.

## Implemented in this phase

### R2 — viewport interaction

- Bevy camera ray-picks graph-owned entities by stable scene IDs. The current hit test uses conservative bounding spheres, not exact triangle intersections.
- Viewport selection updates graph and inspector. **Frame in viewport / F** focuses the selected graph object.
- Viewport overlay now supports **Move (W)**, **Rotate (E)** and **Scale (R)**, independent X/Y/Z changes, snapping and one undo transaction per gesture. These are still 2D interaction handles; depth-aware handles physically anchored to meshes remain outstanding.

### R2 — browser GLB import and persistence

- New `asset.glb` typed node with a SHA-256 `asset_ref`, authored transform, visibility and spin. Existing bundled `asset.boombox` projects retain backwards compatibility.
- Browser imports **binary glTF 2.0 .glb** up to **50 MB**. Validates header, hashes content, writes the bytes and metadata to IndexedDB, then uploads them to an in-memory Bevy asset source called `uploads`.
- Bevy registers `MemoryAssetReader` before `AssetPlugin`, so `WorldAssetRoot` can resolve `uploads://<sha256>.glb#Scene0`, preserving the scene hierarchy and glTF-authored materials.
- `Import GLB` adds the model to the graph and connects it to a Scene when available. The Assets palette lists previously imported models for reuse in more scenes. On browser restart, the editor hydrates referenced GLBs from IndexedDB into the WASM-backed source before loading the saved graph. Missing assets generate an explicit relink/reimport error.
- Resource IDs are validated as lower-case 64-character hex to prevent untrusted traversal or arbitrary URL access. Uploaded bytes must have GLB signature/version/length.
- Asset storage is **local to this browser/origin**; exporting JSON does not embed GLB bytes. Preserve source GLBs for backup and cross-device migration. GLTF with external linked dependencies is not supported in this initial import path; use self-contained GLBs.

### R3 — procedural authoring

- New **Twist Deformer** accepts procedural Geometry, emits Geometry, and modifies real mesh vertices and approximate normals using angle, axis and falloff. Works as a standalone scene object or as the source mesh for any existing cloner. Missing input is an explicit incomplete status.
- New seeded **Random Effector** has a typed Effector output, connected to a cloner's Effector input; uses deterministic per-instance jitter and scale variation, seeded through integer hash. Works in native built-in, unbatched and custom GPU instance preparation. One-click assignment in inspector.
- Existing Grid and Radial distributions and multiple primitive generators from the earlier R1/R3 work are retained.
- Scene objects cache their material descriptor so transform-only frames do not trigger redundant StandardMaterial asset writes to the GPU.

### R4 — time, keyframes and playback

- Backwards-compatible optional `Document.tracks` with validated numeric keys, 0–120s range and linear/step interpolation. Legacy JSON documents need no migration.
- Graph compiler maps supported animation curves to live geometry transforms and PBR roughness/metallic/emissive strength. Authored parameter defaults never become sampled values.
- Bevy `PlaybackClock` owns play/pause, speed, time, duration and loop transitions, used for LFO post effects, graph model spin and CPU clone animation.
- New custom GPU clone vertex input carries a *time multiplier + offset*. Between user transport events, GPU time advances using Bevy's global time **without per-frame instance-buffer uploads**. On play/pause/seek/rate and loop changes, one new instance-buffer upload synchronizes to the chosen time.
- UI has a compact time transport, seek bar, speed and duration, diamond keyframe buttons, interpolation selection and per-node track listing/removal.
- Engine telemetry reports playback position and speed.

## Verification

Automated checks now include graph validator/compiler tests (including imported asset IDs, seeded random, Twist chain, timeline roundtrip and sample values), native Bevy resource and mesh tests, and Chromium browser interactions for graph editing, GLB import/persistence/reuse and keyframes. Browser tests intentionally mock WASM; therefore they **cannot prove actual WebGPU appearance**.

### Target-device acceptance tasks

1. Import a self-contained real GLB with textures; confirm it loads in the viewport. Move, rotate, scale; save and refresh, and confirm hierarchy/materials survive. Add the same asset again from Assets.
2. Select one model by clicking it in the viewport. Frame it with F. Change the overlay's Move/Rotate/Scale modes and snapping; undo changes.
3. Wire Cube → Twist → Grid Cloner. Change Twist angle and axis, then connect Random Effector and vary seed/jitter. Compare native and custom clone modes for positions/appearance.
4. Select a Cube and keyframe its X position at 0s and 2s. Pause, scrub to 1s, and verify visual intermediate position; change interpolation to Step and verify hold.
5. Run 10,000 custom clones, pause, seek, resume and change speed. Verify GPU motion follows the transport and does not require continual instance uploads.
6. Report any WebGPU/asset errors using Debug → Copy report; test macOS Chrome and a page refresh.

## Outstanding (do not claim otherwise)

- **Full spatial 3D gizmos** (mesh-anchored, depth-aware axis handles, local/world mode and precise triangle picking), preview vs authored camera separation.
- **Multi-cloner output** (compiler currently uses one active `scene.compose.clones` input) and GPU-only group transform uniforms; editing a group still rebuilds the instance buffer.
- **Full per-node/port generic graph evaluation** and arbitrary numeric input modulation. LFO effects are still a specific supported set; keyframes cover geometry transform + three PBR scalars, not all numeric inputs.
- **Nonlinear animation curves/timeline editor**, cross-composition asset manifest bundling, drag-drop GLBs into specific ports, missing-asset relink UX and resource GC audits.
- **Automated, real WASM/WebGPU screenshot tests** and reliable GPU device-loss visual acceptance. Native and WASM checks plus mocked Chromium tests are necessary but insufficient.
- High-volume GLB asset validation (texture decompression, large mesh budgets) and multi-submesh clone instancing have not been stress-tested.

## Recommendation

Continue in this order: (1) real macOS Chrome/WebGPU smoke and render-state comparisons, (2) full 3D gizmos + camera isolation, (3) typed multi-cloner instance sets with GPU group uniforms and resource cleanup, (4) broader generic modulation and timeline curves, (5) portable asset bundles. Ship no more unsupported UI toggles until each changes actual rendered pixels.
