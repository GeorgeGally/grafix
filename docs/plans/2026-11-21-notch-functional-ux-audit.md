# Grafix × Notch 2026.2 — Functional and UX Gap Audit

**Audit date:** 21 November 2026  
**Audited Grafix ref:** `feat/typed-graph-editor` @ `f4e2e2cb0b82568ea60768d895509729e7be3028`  
**Other branch:** `main` has not incorporated these editor changes.  
**Review method:** static audit of the Rust graph schema/compiler, Bevy scene/instance/command/camera systems, WASM bridge, Svelte editor/graph/viewport, tests and project plan; comparison with the official Notch 2026.2 manual. No claim of a locally verified WebGPU frame.  
**CI at audited ref:** [passing checks](https://github.com/GeorgeGally/grafix/actions/runs/44521755937), including 10 Playwright tests that **mock the WASM bridge**.

## Decision

**Stop expanding the palette until Grafix has a trustworthy, unified authoring loop.** Its technology stack and first multi-object work are promising; the current code nevertheless retains two sources of truth, a partial scene intermediate representation, hardcoded demo geometry, and controls whose graph connections do not always correspond to their rendered effects.

Do not recreate Notch's entire breadth. Adopt its **interaction grammar**—rapid graph authoring, direct viewport manipulation, rich linked properties, discoverable resources, time and responsive feedback—while keeping Grafix's typed Rust graph, WebGPU rendering and explicit dataflow.

## What actually works (credit where due)

- Versioned JSON composition and typed ports, cycle checking and port validation, topological sorting and an active-output reachability pass.
- Live compiled graph controls for a single cloner, post-FX and camera, with one bloom LFO special case.
- Multiple graph-owned standalone cubes/models/directional lights with stable node IDs, per-object transforms and per-cube PBR base colour/metallic/roughness/emissive descriptor.
- Svelte Flow canvas, add-by-click/drag, search, numeric/enum/bool/HTML colour controls, XYZ inspector, undo/redo, JSON import/export and localStorage persistence.
- GPU instancing benchmark paths, GPU error reporting, diagnostics and WebGPU capability collection.
- A bundled two-colour-cube preset and 10 real-Chromium **UI** tests, albeit with a fake renderer.

Do not call these a complete Notch-like node runtime. There is no generic evaluated geometry/material/float graph, general GLB resource import, raycast picking/3D gizmo, animation timeline or production render tests yet.

## Prioritized findings

Each finding names the evidence, user impact, recommended change and a minimal acceptance gate. Priority is a product/engineering judgment, not a measured score.

### P0 — correctness and trust

#### F01. Conflicting scene controls; no single authoring source of truth
**Evidence:** [legacy App Scene/Clones controls](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/src/App.svelte#L318), [direct set_param/instancing API](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/bridge-wasm/src/lib.rs#L73), [separate ApplyGraph route](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graphics-bevy/src/commands.rs#L279).

The right sidebar still sets engine parameters and instancing directly while the node graph maintains separate saved parameters. An interaction through one path can be overwritten by a subsequent graph edit through the other path. The sidebar's initial control values are only synchronised once from telemetry, then can drift.

**Change:** Put all editable project properties through the graph transaction/action API. Remove legacy Scene and Clones controls from creative mode; retain them only in an explicitly isolated benchmark/debug panel. Make engine-derived *evaluated* values read-only. A single undo/redo stack owns edits.

**Gate:** Edit clone count or bloom in either visible editor surface, make an unrelated graph edit, undo, save/reopen: a single value, with no jumps or divergence.

#### F02. Graph scene is only partially authoritative
**Evidence:** [hardcoded startup GLB, key/fill lights and floor](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graphics-bevy/src/scene.rs#L87), [legacy Controls shape](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graph-core/src/lib.rs#L707), [scene assembly](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graph-core/src/lib.rs#L807), [new graph entity diff](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graphics-bevy/src/scene.rs#L342).

`GraphSceneState` supports several scene items/lights, but the compiler still emits global legacy fields for the first model/cube/light, one cloner and one field. The startup scene still instantiates a BoomBox, floor and demo lights; some become hidden/zeroed rather than being absent from a graph-owned scene. The ambient light/floor remain authored outside the graph.

**Change:** One typed `EvaluatedScene` (items, geometry/material/asset handles, lights, cameras, instance sets, environment and render settings). The existing Gate A demo becomes a loaded *project* rather than unconditional startup entities. Remove or adapt legacy `Controls` only after compatibility tests pass.

**Gate:** An empty scene shows no unrequested objects, and deleting an object removes its GPU/render resources; two independently lit objects survive camera motion and save/reopen.

#### F03. Connected nodes can still be visually ineffective
**Evidence:** [material_for applies only to standalone cube](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graph-core/src/lib.rs#L748), [cloner source handling](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graph-core/src/lib.rs#L930), [hardcoded clone mesh/material](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graphics-bevy/src/instancing.rs#L97), [Svelte 'connected to render output' reachability indicator](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/src/lib/GraphEditor.svelte#L37).

A PBR Material linked to a cube that feeds the Fibonacci cloner is upstream of Output, so the editor can call it active. But the instancing renderer still uses its own cube mesh/material and colours. This is a confirmed code-path mismatch: graph topology alone does not prove a parameter affects pixels. `math.add` remains accepted in saved graphs without an evaluator (hidden from the palette but not executable).

**Change:** Node output types must represent *actual* evaluated resources. Give each connection a capability/implementation status; prevent unsupported linking (or explicitly mark it unavailable). A node inspector must say `Connected & evaluated`, `Connected but unsupported`, `Disconnected` or `Error`—not just reachable.

**Gate:** Connecting a red material to the geometry fed into a cloner produces red clones in both GPU and fallback render modes; no normal-palette node has an inert output.

#### F04. No credible visual runtime acceptance test
**Evidence:** [browser CI copying mock_bridge_wasm.js](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/.github/workflows/ci.yml#L48), [Playwright UI test suite](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/tests/graph.spec.ts#L1), [native/wasm compile-only checks](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/.github/workflows/ci.yml#L12).

CI establishes graph compiler correctness, compilation and many interface interactions. It does **not** render a real Bevy/WebGPU frame and compare actual objects/lighting/post-FX against assertions. A control can pass every browser test while not affecting the GPU output.

**Change:** Add a real built-WASM browser smoke suite (Chrome/SwiftShader where possible) plus native reference renderer screenshots and golden-scene comparisons. Include asset load, 2-cube scene, changed material, 2 lights, cloner mode and camera transform. Document GPU availability differences and manual Mac validation.

**Gate:** A CI integration job fails if material edits, visibility changes or instance count fail to produce corresponding runtime state/screenshot changes. Store actual device/browser metadata.

### P1 — usability and authoring speed

#### F05. Viewport navigation is not object editing
**Evidence:** [orbit camera system](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graphics-bevy/src/orbit.rs#L47), [top-right drag overlay](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/src/App.svelte#L263), [selection events from graph only](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/src/lib/GraphEditor.svelte#L521).

The small X/Y/Z overlay updates numeric position but is not physically attached to the selected object. There is no click-on-mesh selection, hover highlight, depth-aware translate/rotate/scale gizmo, local/world mode, pivot placement or snapping. The same camera is controlled by mouse orbit and authored graph settings; edits can diverge until the graph resets them.

**Change:** Add GPU/CPU ray-based picking, stable graph entity IDs propagated into viewport hit results, viewport selection sync, depth-aware gizmos, W/E/R or E/R/T shortcuts, optional snap, frame selected and two separate camera states (editor preview vs authored output camera).

**Gate:** Click cube A in viewport → A selected in graph and inspector. Move/rotate/scale it with a gizmo → values update in document and undo as one gesture. Cube B is unchanged. No accidental orbit while manipulating.

#### F06. Fixed cramped panels and weak inspector ergonomics
**Evidence:** [fixed 54/46 viewport/graph split and 300px diagnostics sidebar](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/src/App.svelte#L456), [220px inspector, fixed 150px palette](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/src/lib/GraphEditor.svelte#L688), [generic slider/color control markup](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/src/lib/GraphEditor.svelte#L613).

There are two simultaneous inspector concepts: a right-side legacy Scene/Clones panel and a narrow inspector inside a graph that occupies 46% of the stage. The generic number control is a slider with a two-decimal display, not a precise numeric scrub/input. Parameter groups are headings rather than collapsible categories; no property search, default-per-property reset, tooltip, favourite or unit-aware entry. HTML colour inputs quantise to 8-bit hex and cannot author HDR channels.

**Change:** A resizable/dockable 3-pane `Viewport | Graph | Inspector` environment with responsive presets; tuck Diagnostics and Benchmarks behind a developer tab. Build a reusable property editor with numeric input + drag scrub, per-property reset, units/precision, section collapse, search, booleans and sRGB/HDR-aware colour controls. Provide tooltips and contextual docs.

**Gate:** Exact numeric entry and undo work without using a range slider; inspector remains usable at laptop resolutions; user can enlarge viewport/graph independently; the live value is never ambiguous.

#### F07. Slow node authoring and low graph legibility
**Evidence:** [static node cards/ports](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/src/lib/GraphNode.svelte#L14), [palette click/drag and SvelteFlow binding](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/src/lib/GraphEditor.svelte#L498), [limited keydown handler](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/src/lib/GraphEditor.svelte#L474).

Missing node renaming, duplicate/copy/paste, graph groups/comments, floating cursor search, insert-on-wire, one-click add-and-auto-connect, minimap, frame-selection, align/auto-layout, connection hover explanation, per-node preview, mute/bypass and typed error badges. Currently the palette is a skinny fixed column, and new nodes often require separate manual connection actions.

**Change:** Command palette (`Tab`/`Cmd+Space`), socket drag → compatible-node search → auto-insert, double-click empty canvas to add, right-click context, duplicate and copy/paste, group frames/comments, minimap and keyboard navigation. Indicate active, inactive, unsupported and compile error directly on nodes and edges.

**Gate:** From a blank composition, create, connect, rename, duplicate and arrange two objects without leaving graph space or searching through a sidebar. Invalid connection explains why at the pointer.

#### F08. Resource management is missing
**Evidence:** [GLB Model is actually the hardcoded BoomBox path](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graphics-bevy/src/scene.rs#L145), [asset.boombox registry title](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graph-core/src/lib.rs#L190), [JSON-only import](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/src/lib/GraphEditor.svelte#L447).

No asset browser, file import, thumbnail, file status, multi-mesh selection, material override, drag-drop from Files/Finder, persistent resource IDs, asset missing/relink, or import presets. Saved JSON references only bundled resources.

**Change:** Build an asset service first (resource ID/content hash, IndexedDB-backed browser storage, MIME/schema validation, clear missing states and relinking). Add GLB/GLTF drag-drop and material-aware scene hierarchy import as the first vertical. Resource browser with thumbnails, loading/error badges and "drag resource onto graph creates appropriate node". Preserve original glTF materials and submesh hierarchy.

**Gate:** Import two different GLBs, place independently, save/reopen after browser refresh, preserve meshes/materials/textures and surface a missing-asset error rather than silently falling back to the BoomBox.

#### F09. No shared animation/time/parameter modulation layer
**Evidence:** [LFO only connected to bloom](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graph-core/src/lib.rs#L966), [special-case tick_graph_modulation](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graphics-bevy/src/commands.rs#L321), [spin/bob based on elapsed time](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graphics-bevy/src/instancing.rs#L286), [no timeline in editor](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/src/App.svelte#L250).

No reliable pause, scrub, playback rate, reset-to-time, keyframes, curves, parameter binding for arbitrary numeric inputs, authoring vs evaluated values or deterministic re-render at a chosen time.

**Change:** Shared `TimeState { playing, t, rate, loop, frame }`, typed parameter-address binding and evaluator with authored/base and evaluated/modulated values. First support general LFO → Cube.position_y, material.roughness or Cloner.count; then add timeline/keyframes/curve editor as distinct UI. Keep external time sources for later.

**Gate:** Pause, seek to a frame, and reproduce the exact same scene; two different properties can be driven from the same LFO; saving does not overwrite base parameter values with sampled values.

### P1/P2 — creative breadth and performance

#### F10. Too few real geometry generators / material capabilities
**Evidence:** [registry primitive cube only](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graph-core/src/lib.rs#L202), [material descriptor fields](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graph-core/src/lib.rs#L677), [shared unit-cube mesh](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graphics-bevy/src/scene.rs#L171).

Add plane, sphere, cylinder and torus with dimensions/subdivisions/normals, then geometry transforms/pivot and material inputs. Extend PBR with alpha, texture/resource slots, normal map, emissive maps, shadow cast/receive settings and controlled display modes. Do not expose bevel/subdivision sliders until the mesh builder implements them.

**Gate:** Every exposed geometry parameter changes actual vertex/UV data; each material parameter changes rendered pixels for its own object; editing one mesh leaves the other unchanged.

#### F11. Cloning is still a specialized benchmark
**Evidence:** [hardcoded Fibonacci sphere/radius/layout](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graphics-bevy/src/instancing.rs#L121), [single global clone projection](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graph-core/src/lib.rs#L930), [full despawn/rebuild on transform or mode change](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graphics-bevy/src/instancing.rs#L143).

Only one cloner/field is routed into the legacy Controls projection; cloned geometry/material do not compose like standalone renderables. Motion/spin and position are baked in; any cloner/source spatial transform change regenerates the set. Rendering 10k+ clones this way during interaction risks expensive uploads and stalls.

**Change:** General `GeometryHandle -> InstanceSetHandle`, with group transform GPU uniform independent of per-instance distribution buffer. Add purpose-built Grid then Radial, and Random Effector with seed and falloff. Support material propagation, multiple cloners and a clear GPU fallback for multi-submesh imports.

**Gate:** Two cloners can be independently placed/materialized; moving a group does not recreate instance buffers; CPU fallback and GPU path agree on deterministic seeds and transforms.

#### F12. Field node name and UI overpromise
**Evidence:** [`field.compute2d` described as procedural texture](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graph-core/src/lib.rs#L240), [fixed scene panel emissive texture](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graphics-bevy/src/scene.rs#L188).

Notch Fields are actual 2D/3D voxel simulation systems with roots, emitters, affectors and renderers. Grafix currently exposes a GPU-generated texture displayed on a rectangle. Rename/category it as `Texture / Procedural Field` and support output preview; reserve `Field 2D Root` and `Field 3D Root` names for actual solvers.

**Gate:** Users can correctly distinguish a texture generator from a fluid/voxel field before adding it; preview updates with frequency and time.

#### F13. Re-evaluation and GPU resource lifetime need granular invalidation
**Evidence:** [set_node_param clones/recompiles document](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/bridge-wasm/src/lib.rs#L136), [apply graph copies whole scene descriptor vectors](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graphics-bevy/src/commands.rs#L279), [sync_graph_scene revisits all scene items/materials](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graphics-bevy/src/scene.rs#L342), [instance rebuilding](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/crates/graphics-bevy/src/instancing.rs#L143).

The current small-scene implementation is acceptable as a spike; as complexity grows, each slider edit revalidates/recompiles the graph and may visit unrelated entities or reapply materials. Ownership and cleanup of mesh/material/texture GPU resources after deletion/replacement need tests.

**Change:** Per-node dependency graph cache and dirty bits, targeted scene diff commands, stable material/mesh handles and resource reference counts/cleanup tests. Performance telemetry should name which nodes evaluated, what was rebuilt and what uploaded.

**Gate:** Editing object A's material leaves object B's mesh/material resources unchanged; interactive cloner group drag does not re-upload instance buffers; repeated add/delete does not continually grow live resource counts.

#### F14. Incomplete error feedback and project lifecycle
**Evidence:** [validConnection checks only types](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/src/lib/GraphEditor.svelte#L348), [error surfaced globally in toolbar/inspector](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/src/lib/GraphEditor.svelte#L482), [localStorage/JSON-only project persistence](https://github.com/GeorgeGally/grafix/blob/f4e2e2cb0b82568ea60768d895509729e7be3028/apps/editor/src/lib/GraphEditor.svelte#L131).

Cycles, unsupported operations and multiplicity are rejected after a connection attempt, but not explained at the socket. No per-node warnings, compatibility matrix, project version migration, file-backed asset repair, autosave history or user-facing crash recovery. Rapid colour input generates full graph edits and can flood the undo stack; slider keyboard gestures may not be batched like pointer drags.

**Change:** Validate potential connections before committing; inline diagnostics by node/port; one project state store with typed commands and gesture-scoped undo. Autosave and recover validated project + assets atomically. Explicit version migrations and import repair.

**Gate:** Bad graph edit never changes the last working scene; user sees local cause, can undo exact action and reopen identical visual result.

## Workflow comparison with official Notch 2026.2

| Notch capability | Verified Grafix state | What to adopt, not copy |
| --- | --- | --- |
| Graph floating add/find, hints, minimap, align/auto-arrange, embedded nodes | Static palette + connection canvas; no equivalent composition tooling | Add-on-wire search, hints, minimap, groups and frame selection |
| Properties: search, reset, lock, preset, colour/range widgets, keyframe/linking | Flat group headings, range sliders, HTML colour, XYZ numeric inputs; no search/lock/keyframe | Reusable property framework with accurate evaluated value and precise input |
| Viewport gizmos/navigation, selection and camera switching | One Bevy orbit view and screen-corner X/Y/Z buttons | Raycast picking, transform gizmos, editor camera separate from render camera |
| Resource browser / imported 3D Object & Scene | Fixed BoomBox only, JSON project open | Content-hashed browser asset library, thumbnails, GLB hierarchy, relink |
| Cloners and effectors, multi-object clone sources, falloffs | One Fibonacci cube cloner with hardcoded colour/layout | Grid/Radial nodes; reusable GPU instance source/material/effector model |
| Deformers update on change, render / material hierarchy | No deformers or generalized geometry evaluator | One Twist node with correct dirty evaluation before many deformers |
| Timeline, curve editor and property modifiers | No timeline, LFO special-cased to bloom | Unified time and typed numeric parameter bindings |

Primary Notch sources, consulted 21 Nov 2026:
- [Nodegraph UI (updated 4 Nov 2026)](https://manual.notch.one/2026.2/en/docs/reference/user-interface/nodegraph/)
- [Properties](https://manual.notch.one/2026.2/en/docs/reference/user-interface/properties/)
- [Viewport Handles](https://manual.notch.one/2026.2/en/docs/reference/user-interface/viewport/handles/)
- [Keyboard Shortcuts](https://manual.notch.one/2026.2/en/docs/reference/user-interface/keyboard-shortcuts/)
- [3D Object](https://manual.notch.one/2026.2/en/docs/reference/nodes/3d/3d-object/)
- [Material](https://manual.notch.one/2026.2/en/docs/reference/nodes/materials/material/)
- [Cloners & Effectors](https://manual.notch.one/2026.2/en/docs/learning/working-in-3d/cloners/)
- [Deformer optimisation](https://manual.notch.one/2026.2/en/docs/learning/working-in-3d/deformers/)
- [Fields](https://manual.notch.one/2026.2/en/docs/reference/nodes/fields/)
- [2026.2 additions](https://manual.notch.one/2026.2/en/docs/whats-new/everything/)

## Recommended UX model

**One creative workspace, not a demo dashboard:**

`[ Assets / Add ] [            Viewport             ] [   Inspector   ]`
`[ filter       ] [ selection and true gizmos     ] [ properties    ]`
`[ resource     ] [ overlays / camera / play      ] [ keyframe/etc ]`
`[ browser      ] [---- draggable divider -------] [              ]`
`[              ] [ Graph + quick add / previews  ] [              ]`
`[              ] [ minimap / groups / typed pins ] [              ]`

- A single project/document owns all creative parameters and scene actions.
- The graph and viewport share selection, history, and actual evaluated node status.
- Resize or collapse any region. Offer "Create", "Animate" and "Debug" workspace presets instead of separate competing editors.
- The inspector is a consistent property system for all nodes; diagnostics and benchmarks move to Debug.
- The time bar is a future cross-cutting tool, not bolted independently onto every widget.

## Recommended execution sequence

| Phase | Deliverable | Difficulty (judgment) | Exit criterion |
| --- | --- | --- | --- |
| **R0 — Trust** | Graph-authoritative values; isolate benchmark controls; fix material-to-cloner semantics; inline errors; real renderer smoke fixture | Medium–high | Every active connection produces the expected output, and a rendered test proves it |
| **R1 — Authoring UX** | Resizable layout, inspector search/precise numeric controls, rename/duplicate/copy/paste, add-on-wire, minimap, visible active/error state | Medium | Whole first scene made quickly without manual pane switching |
| **R2 — Direct manipulation / Assets** | Click selection, gizmos/local-world/frame; resource browser, real GLB import and persistence, separate preview camera | High | Imported 3D object can be selected, moved, materialized and saved/reopened |
| **R3 — Creative expression** | General geometry/material handles, more primitives, Grid/Radial/Random Effector, one Twist deformer, dirty updates | High | Two independently shaded cloner groups + deforming shape, without full buffer rebuild on group movement |
| **R4 — Animation** | Unified TimeState, generic parameter binding, timeline/keyframes/curve editor | High | Seek/pause/replay produce deterministic output |

**Defer** particles, 3D fluid/voxel solvers, audio/MIDI, physics, advanced post-FX and broad video export until R0–R3 reliably work. They depend on the same asset, execution, transform and time contracts.

## First issue-sized implementation PR

**PR: "Graph owns the scene" (R0a)**

1. Remove editable Scene/Clones debug sliders from the main workspace; retain separate opt-in Benchmark mode.
2. Introduce `ProjectState` transaction API that owns graph edits, undo/redo, autosave and error reporting. All editor surfaces call this API.
3. Build a graph-execution capability check; propagate source materials into clone render data, or explicitly reject/mark unsupported geometry→clone material links.
4. Replace the corner "connected" indicator with execution state derived from evaluated node outputs and include per-port diagnostics.
5. Add a golden scene test that edits material, transform and clone source and checks renderer state (not just a fake WASM bridge).
6. Add user-visible empty scene / add-first-node startup with no leftover demo geometry.

**Do not start new primitive/cloner/field types in this PR.**

### Acceptance tasks the user must be able to perform

1. **Blank scene:** add a cube from cursor search; have it appear connected/visible; edit scale/colour precisely; click it in viewport; undo; reload; identical result.
2. **Real resource:** drop a real GLB; see import status/thumbnail and model hierarchy; connect it; move in viewport; save/reload without losing resource.
3. **Procedural scene:** a cube's red material flows through a cloner; the cloner can use Grid or Radial layout; move group without instance buffer reupload; changing seed changes layout deterministically.
4. **Timing:** animate one parameter with an LFO, pause and seek; value and pixels reflect the chosen time exactly.

## Alternative approaches considered

**A. Copy Notch's node and property catalogue.** Looks rich quickly but duplicates stub functionality and debt; low reversible investment once project files rely on inert nodes. Reject.

**B. Polish Svelte layout first.** Immediate UX perception improvement at moderate effort, but does not fix silent graph/render mismatches. Keep as R1 after trust.

**C. Vertical authoring loop with executable contracts and UI improvements.** Highest compounded value, harder first step, reversible by adapting Gate A benchmark and maintaining `format_version=1` compatibility. Choose.

**Unusual but useful tactic:** Treat each shipped composition preset as an *executable product specification*. Record the graph, screenshots, expected evaluated values, expected draw/resource counts and undo/redo behaviour. A new control is only "shipped" when at least one golden composition asserts its real visual effect.

## Risks / test strategy

- Don't remove the Gate A benchmark; isolate it from authoring state and keep performance comparisons.
- Maintain JSON v1 migration path and alias old node IDs; don't silently reinterpret old project files.
- Imported GLB multi-mesh/material instancing requires a measured fallback path; don't promise custom GPU batching until validated.
- Color picker HTML-only precision isn't enough for HDR authoring; track linear vs sRGB explicitly.
- A full-graph compile on every mouse move, clone regeneration on translate, and updating all material assets can cause scale-dependent stutters. Measure dependency invalidation/instance uploads before optimizing.
- Browser Playwright tests against a mock bridge are necessary but **not** sufficient. Keep them, then add real renderer screenshots / device info.
- Existing macOS Bevy compile checks do not prove Mac Chrome visual output; that remains unverified until run on target hardware.

## THE ONE THING TO DO NEXT

**Implement R0a, "Graph owns the scene," before adding more nodes.** The smallest meaningful demonstration is an empty scene in which two independently materialized objects and one cloner agree across graph, inspector, viewport, undo and saved project—and no disconnected control or demo entity overrides them.
