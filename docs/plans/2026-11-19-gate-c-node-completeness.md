# Gate C — Complete, Executable 3D Nodes
**Date:** 19 November 2026
**Status:** Implementation proposal; no code changes in this document
**Repository audit:** `feat/typed-graph-editor` at `5e43a3b35f09932e3a2a38f345939341d7838d25`
**Target:** Notch-inspired depth of controls with TouchDesigner-style explicit dataflow, not a wholesale Notch clone.

## Executive decision

**Build one genuinely complete 3D authoring loop before expanding the node palette.** Grafix's weakness is not a shortage of inspector fields: it is that most nodes still compile into one global demo scene and hard-coded assets. The next phase must replace that model with typed, per-node scene resources, then add property coverage with actual renderer execution.

**Definition of complete:** A user creates a node, adjusts every exposed parameter, connects it to the graph, sees a measurable and predictable effect in the live viewport, saves/reopens it, and gets an understandable error for unsupported connections. There are no enabled inert controls or fake node types in the normal palette.

**Scope:** Finish Gate B's deferred functionality and deliver the original Gate C hero composition. Keep particles, 3D voxel fluids, physics, audio and full Notch parity out of this phase.

## 1. Verified code audit — what is actually running

### 1.1 Architecture, not just GUI

- `crates/graph-core/src/lib.rs` exposes **13 registered node types**. The registry currently supports `number` and `enum` parameter kinds, plus nine spatial numbers on a handful of nodes. It has no general metadata for groups, units, booleans, colours, vectors, curves, files, visibility conditions, or per-parameter modulation.
- `compile()` resolves a `render.output` through a single `scene.compose` and its **one slot each** for model, geometry, clones, light and field. It turns the result into a single `Controls` struct with fields such as `model_visible`, `cube_transform` and `clone_count`. This does not support two independently editable objects, two distinct materials or two lights.
- `crates/graphics-bevy/src/scene.rs` unconditionally spawns the demo BoomBox, one cube, a grid floor, two colourful point lights, a key directional light and a compute-texture panel. `DEMO_MODEL` is a constant. Most renderer state is fixed even when graph nodes are removed.
- `crates/graphics-bevy/src/instancing.rs` makes only cube-based Fibonacci-shell clones. The layout shape, random seed, radius range, clone colour, bobbing amplitude/speed and spin are hardcoded.
- `crates/graphics-gpu/src/custom_instancing.rs` and the matching shader do have a GPU instance path, but it is still bound to the demo cube mesh and non-general attribute semantics.
- `crates/graphics-bevy/src/orbit.rs` supports orbit/pan/zoom and graph target/distance/yaw/pitch, but no authored projection/FOV/clip plane, and no true object-picking transform gizmos.
- `crates/bridge-wasm/src/lib.rs` handles validated JSON loads and single numeric updates, but every edit recompiles the whole projection. There is no per-node cached evaluation or binding of arbitrary Float outputs to arbitrary parameters.
- `apps/editor/src/lib/GraphEditor.svelte` renders custom XYZ input controls and a generic flat number/enum loop. Graph search, links, import/export and undo exist, but inspector schema and backend capabilities are not yet one executable contract.
- `animation.lfo` only has an actual evaluation path when connected to `fx.bloom.amount`. `math.add` is in the palette but is not an evaluated arithmetic operator; either implement or hide it.
- Gate B tests focus on document validation, Svelte interaction and bridge compilation. They do not yet prove every visible control changes the *correct node-specific render resource* in a real WebGPU frame.

This means **a field appearing in the inspector is not sufficient evidence that it works.**

### 1.2 Notch 2026.2 lessons worth copying

The official Notch reference uses recurring **Transform, Parent Transform, Pivot Point, Attributes, Materials/Rendering, and Time** property groups. A primitive exposes shape/subdivision details, material input and visibility; a cloner exposes a distribution, seed, scale, per-clone transforms and effectors; cameras expose FOV/clip/DOF; lights expose colour/intensity/shadows; materials expose PBR channels and maps; deformers expose strength, axis, space and falloff. We need the functional grammar, not every specialised option.

Notch's 2026.2 work on parameter arrays, custom shader deformers and continuous cloner effectors reinforces the value of **reusable parameter/modulation machinery**. Those advanced features are not Gate C requirements.

Primary references:
- [Node categories](https://manual.notch.one/2026.2/en/docs/reference/nodes)
- [3D Primitive](https://manual.notch.one/2026.2/en/docs/reference/nodes/3d/3d-primitive/)
- [3D Object](https://manual.notch.one/2026.2/en/docs/reference/nodes/3d/3d-object/)
- [Grid Cloner](https://manual.notch.one/2026.2/en/docs/reference/nodes/cloning/grid-cloner/)
- [Radial Cloner](https://manual.notch.one/2026.2/en/docs/reference/nodes/cloning/radial-cloner/)
- [Material](https://manual.notch.one/2026.2/en/docs/reference/nodes/materials/material/)
- [Directional Light](https://manual.notch.one/2026.2/en/docs/reference/nodes/lighting/directional-light/)
- [Camera](https://manual.notch.one/2026.2/en/docs/reference/nodes/cameras/camera/)
- [Twist](https://manual.notch.one/2026.2/en/docs/reference/nodes/deformers/vertex/twist/)
- [2026.2 changes](https://manual.notch.one/2026.2/en/docs/whats-new/release-notes/)

## 2. Next phase node/property coverage

Only expose parameters that are wired to working runtime features. Separate **Gate C required** from **deferred**.

| Current node / target | Gate C required parameters and ports | Not in Gate C |
| --- | --- | --- |
| `asset.boombox` → `asset.glb` | Asset resource selector/import and status; scene/mesh selection; transform, pivot (centre/custom); visibility, cast/receive shadow; material override and original-material choice; readable bounds | Skinning and rigged animation authoring |
| `geometry.cube` + new `geometry.sphere` / `geometry.plane` / `geometry.torus` / `geometry.cylinder` | Dimensions/radius, appropriate subdivisions and bevel where supported, normal/smoothing option, UV behaviour, transform/pivot, visibility, material input, cast/receive shadow | Full mesh modeller and complex topology tools |
| `clone.fibonacci` | Count, min/max shell radius, Y flattening, seed, global and per-clone size, source/object transform, animated spin and bob controls, deterministic colour option; real source Geometry input | Mesh-surface and image-driven distributions |
| New `clone.grid` | Count XYZ, spacing/total size XYZ, grid origin/offset, seed, axis-specific stagger, fill factor, global/per-clone transform | Arbitrary nested cloners and distribution painting |
| New `clone.radial` | Count, radius, start/end angles, plane, orient-to-tangent, seed, clone offset/scale/rotation | Advanced per-clone UV sheet controls |
| New `effector.random` | Seed, position/rotation/scale randomness, amount and falloff mode, deterministic GPU-compatible stream | Procedural texture/voxel falloffs |
| New `material.pbr` | Base colour, metallic, roughness, emissive colour/intensity, opacity/alpha mode, double-sided and texture slots for base colour, metallic/roughness and normal; typed Material port | Refraction/SSS/raytracing uber material |
| `lighting.directional` | Colour (RGB), intensity, orientation, enabled, shadow on/off, bias, softness where Bevy supports it | Raytraced shadow/contact-light parity |
| New `lighting.point` / `lighting.spot` | Colour, intensity, range; spot inner/outer cones; cast shadows/bias where supported | IES profiles and projected video |
| `camera.orbit` | Target XYZ, yaw, pitch, distance, perspective FOV, near/far clip, ortho toggle/scale, aspect policy, frame-selected action | DOF bokeh and motion blur renderer |
| `scene.compose` | Multi-input SceneItem collection, visible/mute/solo per item, stable object IDs, scene background/ambient if graph-owned | Nested multi-render-layer compositing |
| `render.output` | Output resolution/scale, background, tonemapping mode, exposure, optional screenshot PNG; stateful render diagnostics | Offline film-quality export |
| `fx.bloom` | Enabled, intensity, threshold and soft-knee if supported by chosen Bevy render path | Lens dirt texture/advanced glare |
| `fx.aberration` / `fx.vignette` | Enabled, intensity, spatial center, vignette radius/softness or aberration offset/direction; implement in WGSL before exposing | Arbitrary reorderable Post-FX graph |
| `field.compute2d` | Clearly label **Procedural Texture 2D**; resolution, pattern, scale/frequency, speed, seed, palette/gradient, texture output and preview | Fluid/voxel solver; reserve separate types |
| `animation.lfo` | Frequency, amplitude/range, phase, waveform (sin/triangle/square/saw), time source; connect to *any* compatible numeric parameter | Audio/MIDI/OSC and animation timeline |
| `math.add` | Real float evaluator and optional constants/bindings, or hide from normal palette until it works | Complete mathematical operator library |

A single primitive type can own common implementation and emit separate searchable palette entries. Preserve the `geometry.cube` type ID for saved project compatibility.

**New labels should not imply capabilities the runtime lacks.** For example, an uploaded GLB must not silently render the bundled BoomBox, and a 2D procedural texture must not be described as fluid simulation.

## 3. Work package 1 — executable parameter schema (P0)

**Why first:** Adding 100 floats to today's registry would produce a larger but still misleading editor.

### Contract

Extend `ParamSpec` (or create `ParameterDefinition` in a new `crates/graph-nodes` crate) with:

- Stable ID, label, category/group, display ordering, description/help and default.
- `kind`: `float`, `integer`, `bool`, `enum`, `vec2`, `vec3`, `colour`, `asset_ref`, `curve`. Add component-level editing for vectors/colours without losing atomic values.
- Unit and presentation: degrees, metres, seconds, colour space, precision, slider type, min/max/step and reset.
- Optional `visible_when`/`enabled_when` expression over known parameter IDs; no arbitrary JS execution.
- `animatable`, `modulatable` and domain (`cpu`/`scene`/`gpu`); backend binding identifier and node capability level.
- A consistent visibility/enabled/bypass property with explicit per-type semantics. No fake visibility on a source without a rendered SceneItem.

Rust must validate all types and bounds, including enum values, finite vectors, asset IDs and conditional modes. TypeScript types should be *generated* from Rust schema. Keep the old `format_version: 1` documents loadable; add a tested migration to v2 only when the storage format actually changes. A missing new property must take its schema default, not zero.

### Inspector

Group controls into collapsible **Object / Geometry / Material / Transform / Rendering / Animation / Advanced** panels as applicable. Build real number scrubbers, XYZ inputs, booleans, RGB/HDR colour pickers, enumerations, asset selectors and curve previews. Add tooltip/units/reset/keyboard input. Hide conditional controls rather than leaving inoperative sliders visible. Separate node bypass from graph disconnection.

### Validation gate

Every visible parameter needs a contract test: load a fixture, change it, observe the correct graph/runtime value, save/reload, then reset. Tests must fail when a parameter is declared but has no execution binding. Keep special renderer-configuration parameters explicitly identified as such.

## 4. Work package 2 — per-node Scene IR and asset handles (P0)

Replace monolithic `Controls { model_visible, cube_transform, clone_count, ... }` as the central scene model with typed results:

`NodeOutput = GeometryHandle | MeshAssetHandle | MaterialHandle | SceneItem | InstanceSetHandle | LightDesc | CameraDesc | TextureHandle | Float | Colour`.

`SceneItem` contains stable `node_id`, `instance_id`, transform, geometry/material handles and visibility/render flags. `SceneBuild` accumulates **multiple** scene items, lights, instance sets and a selected camera. Keep node graph validation independent of Bevy, but compile results to an intermediate scene representation rather than one global demo knob struct.

Implementation boundaries:
- `graph-core`: DAG, multiplicity rules (`single` vs `many` inputs), validation and stable node result handles.
- `graph-nodes`: definitions, defaults, capability registry and evaluators.
- `graph-runtime`: topological dirty evaluation, per-node cache, version tracking and dependency invalidation. Distinguish `ParamChanged` from topology changes.
- `graphics-bevy`: idempotent mapping from stable graph IDs to Bevy entity and GPU handles; on deletion, release only the removed resource. Preserve unchanged entities.
- `bridge-wasm`: typed commands/events for graph diff, assets, parameter values, inspector capability/error states. Never round-trip whole large GPU resources through JavaScript.

**Asset spike to run before committing to full import:** import a GLB with multiple meshes/materials; confirm textures/hierarchy/relative transforms survive WASM loading, reload, and reference repair. For user-file uploads, persist browser file bytes separately from graph JSON (e.g. IndexedDB keyed by content hash); saved graphs reference asset IDs rather than ephemeral blob URLs or Mac paths. If dynamic Bevy asset loading needs a custom reader, implement that contract early, not after the UI picker.

**Critical risk:** The current custom instance draw path assumes one cube mesh. General Geometry may require a one-mesh fast path and a multi-submesh fallback. Measure both before advertising arbitrary GLB cloning.

## 5. Work package 3 — geometry, materials, lights and camera (P1)

1. Implement primitive generators as typed, cached Bevy meshes; dimensions/subdivisions only regenerate the affected mesh. Recompute normals and UVs deliberately.
2. Implement material instances and material override ports. Use authored defaults, not demo-wide hardcoded `StandardMaterial` values. Share immutable assets where possible; copy-on-write for individual edits.
3. Implement multiple directional, point and spot lights from graph nodes; stop injecting unrelated always-on demonstration point lights when a real graph is loaded.
4. Implement authored camera projection/FOV/clips/exposure. The viewport navigation camera and output camera should have explicit ownership; editor orbiting must not rewrite the camera node unless the user elects to **Set Camera to View**.
5. Add true object picking and 3D translate/rotate/scale gizmos (W/E/R or equivalent), local/world mode, snapping and frame-selected. Replace the current corner-overlay-only translation metaphor as the primary manipulator.
6. Implement display flags and per-object enable/disable at renderer level; tests must prove disconnected nodes no longer keep lighting or visible geometry alive.

### Vertical acceptance A: two-object material scene

From an empty graph: add two different primitives or a primitive plus an uploaded GLB; place and scale separately; apply different PBR materials; add two graph-connected lights and a camera; change FOV and shadows; save/reload. No unrequested demo BoomBox, cube, lights or field panel remain visible.

## 6. Work package 4 — useful clone distributions and effectors (P1)

Split the Fibonacci Gate A benchmark node from creative distribution operators. Preserve Fibonacci for test/legacy projects, but add **Grid**, **Radial** and **Random Effector** as real nodes.

- Geometry in → InstanceSet out; instance data includes world transform, colour and deterministic seed. Do not create 10,000 ECS entities in custom GPU mode.
- Grid: count X/Y/Z and size/spacing mode, stagger and fill. Guard multiplication overflow and GPU buffer limits.
- Radial: arc, ring radius, plane and tangent alignment; support animated phase without reuploading all instances on every frame.
- Random Effector: position, scale, rotation variation, seed; predictable order of effectors and explicit input/output handles.
- Keep group transforms in GPU uniforms where possible. Avoid current full instance-buffer regeneration on every interactive translate/rotate/scale drag.
- Track custom GPU and built-in fallback parity with deterministic fixture seeds and identical scene values.

### Vertical acceptance B: clone garden

Geometry or GLB → (optional Twist) → Grid/Radial Cloner → Random Effector → Material → Scene → Camera/Render → Output. Count/spacing/arc/seed/material changes update live, without losing stable objects or causing unrelated GPU buffer rebuilds. Benchmark on the user's documented Mac/Chrome reference device; record actual FPS, GPU time, uploads, draw calls and instance count. Targets remain targets, not claims.

## 7. Work package 5 — deformers and broadly usable modulation (P2)

- Implement **Twist** first, then **Bend** and **Noise** with typed `Geometry → Geometry`; control axis, amount, range and falloff; distinguish geometry deformation from merely moving an object.
- For static meshes, cache deformed geometry and regenerate only when inputs change. For continuously animated deformation use a shader/GPU path, preserving correct lighting normals.
- Add generic `Float` evaluation and direct parameter-binding endpoints. `LFO → any Float parameter`, not just bloom; implement `math.add` or remove it until valid.
- Define explicit time source, reset and evaluated-value telemetry. Preserve base authored parameter separately from the modulated runtime value so saving doesn't bake an LFO sample.
- Do not begin the fixed-step GPU particle engine until this shared time and modulation contract is sound.

### Vertical acceptance C: visibly deforming composition

A subdivided primitive twists/bends while independently controlling a material property or clone spacing with an LFO; its normals, bounds, save/reload and parameter reset remain correct. Pausing time freezes animation without disabling static inspector edits.

## 8. Execution order and stopping rules

| Step | Deliverable | Acceptance gate |
| --- | --- | --- |
| C0 | Parameter contract + generated inspectors + migration | All supported parameter kinds validate, render, round-trip; no visible inert controls |
| C1 | Multiple graph-owned SceneItems/materials/lights + asset spike | Two independently controlled objects; editing A doesn't mutate B; no hidden demo dependency |
| C2 | Primitive/GLB/material/light/camera coverage + viewport gizmos | Vertical acceptance A passes in real WebGPU |
| C3 | Grid/radial clone distributions + effector | Vertical acceptance B passes and performance is measured |
| C4 | Twist/bend/noise + generic numeric modulation | Vertical acceptance C passes, animation and project persistence work |
| C5 | Hardening + benchmark and visual fixtures | Regression suite, memory/resource cleanup and Chrome/WebGPU acceptance documented |

**Do not start C3 before C1/C2 establish arbitrary geometry and stable resource handles.** Otherwise the second cloner will be another specialised sphere-of-cubes implementation.

## 9. Test matrix and observability

- **Rust unit:** JSON/registry migration; all param kinds/ranges; multi-edge port cardinality; node reachability; deterministic random; graph results; LFO/Math evaluation; dirty propagation; type mismatch/cycle.
- **Renderer integration:** 2+ independent primitives, 2+ materials, multiple lights, changed camera FOV, imported GLB hierarchy; remove/replace node; ensure only owned GPU resources change.
- **Browser interaction:** colour input, toggle/enum/vector fields, asset upload, reset, keyboard edit, gizmo picking, drag, focus, undo, project reopen; no errors at initial load.
- **Visual goldens:** original Gate A baseline, independent red/blue objects, metallic/rough material pair, grid and radial geometry, deformed shaded torus. Compare screenshots with a documented tolerance.
- **Resource stability:** repeated asset replacement and graph reload, device loss, bad shader/resource recovery, absent assets, oversized instance requests; no silent blank canvas.
- **Performance:** record baseline and new GPU draw/upload counts at 1k/10k/50k instances on the documented browser+device; avoid blanket frame-rate promises.
- **Acceptance report:** a `docs/decisions/gate-c-results.md` linking real screenshots, device/browser versions, test logs, known limitations and measured performance.

## 10. Risks and challenges to the recommendation

| Failure mode | Consequence | Mitigation |
| --- | --- | --- |
| Parameter-first development without executor | Larger inspector, same demo visuals | Executable binding contract plus render-side tests |
| Replacing the existing demo graph abruptly | Old saved localStorage projects break | Node IDs/param default migration tests and compatibility aliases |
| One custom renderer for all imported geometry | Multi-material GLB instances render incorrectly | Early GLB instancing spike, explicit slow/fallback path |
| Transform edits respawn all clone entities/upload buffers | UI stalls on every drag at large N | Separate group transform GPU uniform from instance generation; profile |
| Camera controls conflict with viewport orbit | Values snap back or drift | Separate preview view from authored output and explicit sync commands |
| Accumulating fake feature labels | Palette looks rich but frustrates creators | Register only shipped operators; hide stub math and unreleased effects |
| Trying to match every Notch parameter | Never finishes the first compelling composition | Scope to 3D creator verticals; keep specialised physics/volumes/ONNX later |

## 11. Three implementation strategies considered

1. **Copy Notch's property lists wholesale** — quick apparent breadth, extremely high mismatch risk, low completion probability. **Reject.**
2. **Fix each existing node independently** — low initial effort but duplicates widgets, hardcodes render updates, and requires a rewrite once we need more than one object. **Reject.**
3. **Executable schema + multi-object typed Scene IR + full vertical slices** — higher initial effort, strongest impact and testability, reversible via adapters while maintaining Gate A. **Choose.**

**Strongest counterargument:** This phase looks like infrastructure rather than new visual effects. The response is to make C1 end in an undeniable visual milestone—two completely independent, authored, material-driven objects—then C3 delivers the creative Grid/Radial scene. This is a narrower path to useful Notch-like expressiveness than either more demo knobs or premature particle complexity.

## THE ONE THING TO DO NEXT

Start with **C0 + C1**: add the executable parameter contract and multi-object Scene IR, then prove **two independently styled and positioned objects** can be created from the graph, not from `scene.rs` startup constants. Do not begin another node category until that acceptance case passes.
