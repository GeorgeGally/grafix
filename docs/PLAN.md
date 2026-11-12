Browser-First Real-Time Graphics Engine — Implementation Plan
Status: Proposed architecture and execution plan
Date: 12 November 2026
Reference direction: Notch's visual immediacy and production-quality effects + TouchDesigner's linear, data-oriented patching
Technical foundation: Rust · Bevy 0.20.x · wgpu/WebGPU · WGSL/WESL · WASM · Svelte 5 · Svelte Flow · TypeScript

0. Executive decision
Build a node-based real-time 3D graphics instrument, not a 2D compositor and not a generic game editor. The web app hosts a Svelte interface and an embedded Bevy WebGPU canvas. The project is represented by a versioned, engine-independent typed graph. A Rust compiler/runner evaluates graph operations, builds renderable resources in Bevy, and schedules GPU compute/render passes. Custom shaders power advanced cloning, deformation, particles and fields. Start with complete, attractive vertical compositions crossing the requested categories; expand operator coverage incrementally.

Three engineering choices considered
Choice    Strength    Cost/risk    Decision
Rust + raw wgpu    Maximum control    Rebuild scene, asset, PBR and camera infrastructure    Keep as fallback
JS/Three.js renderer + Svelte    Fast browser integration    Less Rust ownership and less control of custom GPU architecture    Use only if Bevy fails proof gate
Rust + Bevy + custom WGSL + Svelte Flow    Built-in 3D foundation, web path, advanced custom pipelines    Integrating custom GPU state and browser lifecycle    Choose; validate early

Constraints and non-goals: Browser-first with Chrome desktop as the first supported target. Native executable is a later packaging target. Do not promise WebGL2 fallback for GPU compute, Safari/Firefox parity, integrated fluid physics, offline video encoding, or a massive public plugin ecosystem in version 0.1. Do not imitate Notch internals or proprietary assets; use its documented user-facing categories as functional inspiration.

1. Success criteria
1. Create a visually compelling scene with a model or primitive, a deformer, thousands of clones, PBR materials, lights, camera and effects, entirely in the browser.
2. Create a second scene with GPU particles driven by a 3D vector field, rendered as sprites and/or meshes, with independently controllable material and lights as applicable.
3. Make and rearrange connections while the output remains live. Adjust parameters with no full editor redraw and no render-runtime restart.
4. Save and reload a composition without losing graph layout, parameter values, external asset references, animation, and seeded procedural behavior.
5. Make the system feel creative: strong defaults, high-quality presets, immediate visual response, accessible parameters, clear node previews and errors.
Provisional performance budgets, not guaranteed results: Aim at 60 fps at a 1080p canvas on one specified, logged reference machine/browser for the canonical test scenes; benchmark 1k/10k/50k clones and 10k/50k/100k particles to establish realistic operating envelopes; maintain smooth UI interaction under load. Publish measured GPU frame time, CPU time, memory, draw calls and shader compile time rather than relying on headline fps. Begin with the user's actual Mac and Chrome; add a second discrete-GPU reference device when available. These counts are test inputs, not claims of performance.

2. Product model
2.1 Distinct graph systems
- Processing graph (primary): Left-to-right typed node dataflow; the thing the creator edits.
- Scene representation: Entity/material/light/camera handles compiled from graph outputs; not the serialized source of truth.
- Modulation connections: A scalar/vector/color/signal can drive a compatible parameter directly. Modulation is visually distinguishable from geometry/texture flow and does not require chains of conversion nodes for common mappings.
- Time and simulation graph: Stateless procedural evaluation, stateful particle/field updates, and history/feedback are explicitly distinguished. Only an explicit Delay/`Feedback`-type node introduces permitted frame-to-frame cycles.
2.2 Canonical runtime types
Bool, Integer, Float, Vec2, Vec3, Vec4, Color, Quaternion, Transform, String, Curve, Signal, Texture2D, Texture3D, Geometry, MeshAsset, GeometryStream, InstanceSet, VectorField2D, VectorField3D, VoxelField2D, VoxelField3D, ParticleState, Material, Light, Camera, SceneElement, Scene, RenderTarget, Image.

Prefer opaque resource handles for large meshes/textures/particles/instances. Do not copy buffers across the JS/WASM boundary. Introduce typed coercions only where unambiguous (e.g., Float→Vec3 broadcast with a visible indicator); reject invalid links at connect time. A set of nodes can consume/produce SceneElement without losing the underlying type information.
2.3 Node declaration contract
A node plugin provides:
- Stable type_id, schema_version, category, icon, title, description, keywords.
- Typed input/output ports, default parameter values, units, ranges, enum choices and presentation hints.
- Evaluation domain: pure_cpu, scene_builder, gpu_compute, gpu_render, stateful_simulation, or io.
- Dirty/invalidated dependencies; optional reset, checkpoint and deterministic seed semantics.
- Serialization migration handler and render-resource cleanup hook.
- Optional inspector presets, thumbnail strategy and GPU capability requirement.
Generate the Svelte node palette and inspector controls from the Rust-owned registry schema exported as a versioned JSON interface. This avoids maintaining two versions of the node API.
2.4 Example project JSON (illustrative schema; not production API)
{
  "format_version": 1,
  "project": { "name": "Field Garden", "fps": 60, "seed": 1441 },
  "nodes": [
    { "id": "sphere-a", "type": "geometry.sphere", "version": 1,
      "params": { "radius": 0.2 }, "ui": { "x": 60, "y": 100 } },
    { "id": "cloner-a", "type": "clone.grid", "version": 1,
      "params": { "count": [30, 10, 30], "spacing": [0.4, 0.6, 0.4] },
      "ui": { "x": 340, "y": 100 } }
  ],
  "edges": [
    { "from": ["sphere-a", "geometry"], "to": ["cloner-a", "source"] }
  ],
  "assets": []
}
node_id must be stable across graph changes. Resource IDs in Bevy are generated/execution-specific and are never written to the saved project. The graph loads even when an asset or optional node plugin is missing, showing a repairable placeholder rather than discarding data.

3. Repository and execution boundaries
graphics-engine/
├── Cargo.toml                   # pinned Bevy 0.20.x and workspace dependencies
├── rust-toolchain.toml          # pinned tested compiler
├── crates/
│   ├── graph-core/              # types, IDs, validation, graph compile, serialization
│   ├── graph-nodes/             # node definitions and registry
│   ├── graph-runtime/           # evaluation, dirty propagation, time, command handling
│   ├── graphics-bevy/           # ECS mapping, assets, PBR, cameras, render integration
│   ├── graphics-gpu/            # custom instancing, compute, particles, fields, deformers
│   ├── graphics-shaders/        # WGSL/WESL and shader pre-processing
│   ├── bridge-wasm/             # JS/WASM API, browser configuration, events
│   └── graphics-cli/            # native/standalone smoke tests and future headless tools
├── apps/
│   └── editor/                  # Svelte 5, Svelte Flow, inspector, timeline, viewport
├── assets/
│   ├── demo/                    # license-safe glTFs, HDRIs, textures
│   └── presets/                 # authored graphs and compound-node presets
├── tests/
│   ├── fixtures/                # versioned graph JSON and glTF fixtures
│   ├── snapshots/               # golden visual outputs with tolerance policy
│   └── benchmarks/              # documented hardware/browsers and results
└── docs/
    ├── decisions/               # ADRs and spike reports
    ├── node-api.md
    ├── browser-support.md
    └── rendering-architecture.md
3.1 Browser lifecycle and communication
- Use Svelte/Vite to load the WASM package once, and mount Bevy onto a dedicated <canvas> owned by the viewport pane.
- WASM receives commands, not copies of the entire graph for every parameter change: LoadGraph, UpsertNodes, RemoveNodes, Connect, Disconnect, SetParam, SetTransport, SetViewport, ImportAsset, RequestSnapshot.
- Keep a command queue on the Rust side; apply graph changes at a safe frame boundary. Coalesce drag/slider updates and preserve final exact value on pointer release.
- Return typed asynchronous events: GraphCompiled, CompileError, ShaderError, AssetReady, NodeStatus, Diagnostics, SnapshotReady, CapabilityReport.
- Give Svelte the editor graph state; give Rust the validated execution graph. Do not emit live GPU frame data into Svelte reactive stores. Throttle node statistics and thumbnails.
- Prefer a single canvas/main-thread integration for the initial spike. Investigate OffscreenCanvas/worker ownership only if profiling proves the event loop is the bottleneck.
- Handle resize and device-pixel ratio. Allow quality scaling independent of editor zoom.
- Handle WebGPU unavailable/device lost with a visible diagnostic and recover-or-reload path. No silent black viewport.
3.2 Evaluation phases per frame
1. Graph edits: Validate types, inspect version changes, compile affected subgraph, allocate/free resources.
2. Time: Apply transport time, timeline curves, LFOs, data/audio inputs and direct parameter bindings.
3. Pure evaluation: Topologically evaluate only dirty stateless nodes; cache values/resources.
4. Scene updates: Propagate revised mesh/transform/material/light/camera handles into Bevy, keeping stable entities where possible.
5. Fixed-step simulation: Execute particles/voxel fields in a bounded fixed timestep; GPU buffers remain resident.
6. GPU render: Perform main 3D render, then ordered post-process nodes, then presentation.
7. Observability: Record timings and errors, emit sampled node previews/metrics.
A DAG is the default execution graph. Stateful nodes have explicit previous-frame inputs/state. Separate simulation updates from render interpolation so frame rate changes do not silently change physics.

4. Editor UX: linear like TouchDesigner, fast like Notch
Layout
- Center: Fixed live 3D viewport, optionally a split viewport/node graph arrangement.
- Bottom or alternate tab: Left-to-right patch graph, zoomable/pannable.
- Right: Context-sensitive inspector. Parameters grouped into Basic/Advanced with ranges and defaults tuned for beautiful results.
- Left: Searchable node/asset palette plus presets and scene collection.
- Top/bottom strip: Transport (play/pause/reset), time, resolution, FPS/GPU diagnostics, save/export.
Interaction priorities
1. Search-create nodes via keyboard, with fuzzy categories/tags; place node at cursor.
2. Drop a node on an edge to auto-insert it when types permit.
3. Type-valid ports, immediate connection feedback, hover explanation for invalid links.
4. Drag any compatible signal output onto a parameter label/control; visualize mapping, offset, range and clamp.
5. Double-click node to focus/preview; show compact generated thumbnails for visible output-capable nodes only.
6. Click-select, marquee-select, multiselect, duplicate, bypass, delete, undo/redo, copy/paste.
7. Group/compound reusable nodes with exposed inputs, outputs and parameters.
8. Distinguish geometry/image/field/signal/simulation wires using subdued color and line semantics; don't overload interface with bright colors.
9. Keep DOM updates tied to node edits and slow sampled status; GPU updates never require Svelte component rerender each frame.
10. Support viewport orbit/pan/dolly and framing of selected object; keep viewport keyboard/pointer handling separate from Svelte Flow drag handling.
Visual direction: restrained, dense but legible pro-creative software. Prioritize instant viewport changes, minimal chrome and properly tuned preset imagery over giant decorative cards.

5. Node catalog and feature maturity
Legend: Core = needed for first compelling working release; Expand = add once core cross-category flows are stable; Later = deliberately deferred. These are system capabilities, not a fixed limit on node count.
Category    Core nodes/capabilities    Expand    Later
Geometry / 3D    Box, sphere, plane, torus, transform, group, GLB/glTF loader    Cylinder, curve, procedural mesh, merge geometry    Complex model editing, skinning controls
Deformers    Twist, bend, procedural noise displacement    Wave, taper, lattice-ish deformation, normals strategy    Unified GPU deform stack, topology edits
Cloning    Grid, radial, source instance set; random/transform effector    Mesh surface distribution, spline cloner, per-instance field effector    Sophisticated custom distribution and agent behavior
Fields    Procedural vector-field source, curl/noise source, field sample, field force    2D voxel advection, volume visualization    Full 3D voxel fluid / smoke simulation
Particles    GPU particle root, point/sphere emitter, gravity, drag, turbulence/field force, sprite renderer    Mesh emitter, mesh renderer, color/life curves, collisions    Inter-particle dynamics, advanced sorting
Materials    Standard PBR, unlit/emissive, texture inputs    Material presets, procedural textures, custom WGSL    Complex material graph editor
Lighting    Directional, point, environment/HDRI    Spot, rect-area, shadow quality controls    Volumetric advanced techniques
Rendering    Camera, scene collector, renderer, bloom, tonemapping, output    DOF, fog, feedback, color correction, layers    Multi-camera live performance outputs
Data/animation    Time, LFO, noise, arithmetic, remap, keyframes, basic audio FFT    Envelope, curves, MIDI, OSC, event signals    Full TouchDesigner-like data operators

5.1 Avoid false equivalence between field implementations
A procedural vector field provides sample(position,time)->velocity and can animate clones/particles with very little setup. It is not a simulated fluid.

A voxel simulation stores evolving density/color and velocity in 2D/3D textures/buffers, injects forces, advects state and renders volumes. Notch's documented 3D Fields use voxel data and timestep-based simulation. Build the procedural field early so particles work, but reserve typed VoxelField resources and explicit solver stages for the later actual fluid implementation. Do not market the first as equivalent to Notch's voxel fields.
5.2 Cloning and particle performance
- Begin with Bevy automatic batching/instancing and verify the actual draw calls. For large sets, build one custom GPU instance buffer / indirect-capable path as necessary rather than creating tens of thousands of ECS entities.
- InstanceSet should support transform, color, scale, and user attributes. Effectors are ordered, deterministic transformations of the instance stream.
- Store particle position, velocity, lifetime, age, seed and color in GPU storage buffers, using ping-pong buffers where required.
- Emitters produce new particle states; affectors run in a fixed-step compute sequence; renderers interpret the particle buffer. Avoid CPU↔GPU readback on every frame.
- Respect browser buffer size, workgroup, texture format and resource binding limits; detect capabilities at startup.
5.3 Deformer implementation
- Ship useful CPU-side geometry deformation first for correctness on ordinary meshes, but avoid reuploading meshes needlessly every frame.
- Move continuously animated and heavily cloned effects into vertex/compute WGSL where the demo requires it.
- Ensure normals/tangents are handled correctly. Deforming positions while leaving stale normals will look unacceptable under PBR lighting.
- Preserve a non-destructive input geometry resource and clear deformation order. Define what happens with animated/skinned glTF models; don't promise that every importer feature is supported immediately.

6. Milestone execution order and gates
Gate A — Browser/Bevy renderer proof
Build: A Svelte page embeds a Bevy 0.20.x WebGPU canvas. Load GLB/glTF; show PBR material, directional light, camera orbit, HDRI/environment if supported, bloom and a slider wired through JS/WASM to a Rust parameter. Test custom WGSL material and a post-processing pass. Run Bevy automatic instancing, then a low-level instance example. Finally run one minimal WebGPU compute shader with an observable result in the canvas.

Deliverables: Running URL, source, an explicit dependency lockfile, browser/GPU capability report and a one-page spike report comparing built-in instancing vs custom instancing.

Pass gate: GLB renders, live parameter controls work without restarting, one compute pass works, resizing is reliable, clean console on supported Chrome, and GPU/device errors are visible.

Stop/reassess if: Custom compute or render pipeline extension requires invasive continuous patching of Bevy internals, or browser integration cannot sustain interactive performance. Consider raw wgpu for the advanced-rendering module before building the full editor. Do not migrate the whole project reflexively.
Gate B — Typed graph + live editor
Build: Separate graph-core, schema-driven node registry, JSON save/load, stable IDs, type checker, DAG evaluation, dirty propagation, command/event bridge, node search, Svelte Flow patch editor, inspector, drag and undo/redo. Initial graph can edit primitive/GLB → transform → standard material → scene → camera/renderer → output; wire a time/LFO signal to one visible parameter.

Pass gate: The whole graph can be created/edited/saved/reopened; invalid port connections fail gracefully; editing one parameter does not rebuild unrelated graph resources; graph undo/redo works.
Gate C — Geometry, deformation, cloning and appearance
Build: Primitives, GLB loader, twist/bend/noise deformation, grid/radial cloners, transform/random/noise effectors, PBR/emissive materials, lights, HDRI, camera controls, bloom and tonemapping. Add object selection and framing.

Hero composition A: Import a GLB or make a torus; twist/deform; distribute an animated grid or radial arrangement; apply noise effector; light beautifully; bloom and orbit camera. Preset project provided.

Pass gate: Whole scene authored from the graph; cloning count adjustable while running; graph remains editable at benchmark settings; visual result looks polished at default values. Record measured clone count/framerate/draw calls.
Gate D — Fields, particles, animation and audio
Build: Procedural 2D/3D vector fields, field sampler, GPU particle buffers, point/sphere emitters, gravity/drag/turbulence, field force, sprite renderers, color over lifetime, fixed-step simulation, time/LFO/keyframe nodes, browser audio FFT source with user-initiated audio permission.

Hero composition B: A swirling 3D particle cloud follows a curl-noise field, varies color/size over lifetime, and responds to audio spectrum; camera, material and render nodes remain editable.

Pass gate: Particle movement follows a field, not a baked animation; simulation resets cleanly; pause/resume and deterministic seed behavior documented; GPU resources aren't recreated during routine parameter drags; benchmark counts are reported.
Gate E — Advanced depth, quality and presets
Build: Mesh-surface cloner/emitter, layered rendering, camera DOF, color grading, feedback/history, compound nodes, preset browser with screenshots, asset browser, parameter presets, curve editing, missing-asset repair. Introduce actual 2D voxel field with advection, density injection, velocity, dissipation and visualization as an advanced proof before 3D fluid fields.

Hero composition C: Audio-reactive cloned geometry + a particle layer + camera/post-processing with one reusable macro/preset.

Pass gate: New users can create one attractive scene from presets and modify its underlying graph; rendering keeps working when a node is bypassed, substituted or a shader fails to compile.
Gate F — Hardening and packaging
Build: Snapshot rendering to PNG, project+asset export bundle, migration tests, tests for browser device loss, diagnostics/performance overlays, error reporting, automated build checks, static HTTPS deployment. Investigate native Mac build using the same Rust graph/runtime and a thin host rather than rebuilding the creative engine.

Pass gate: A saved demo project and its assets open on a second machine/browser within supported requirements; edits and renders match within documented tolerance; no asset leaks across graph reloads.

7. Acceptance test projects
Fixture A — clone-garden.graph.json
- Graph: glTF/primitives → deform → grid/radial cloner → effector → material → scene → PBR render → bloom → output.
- Expectations: correct GLB asset loading; per-instance transforms; responsive changes to clone count; consistent normals; material/lighting; reasonable performance under increasing instance counts.
Fixture B — field-particles.graph.json
- Graph: procedural vector field → field force → particle simulation → particle renderer → scene → camera/render/output; time/audio signal modulates emission.
- Expectations: visible swirl, proper pause/reset, stable GPU buffer resource counts, deterministic-seed replay under the same fixed timestep, no per-frame readbacks.
Fixture C — creative-instrument.graph.json
- Graph combines cloning, deformer, lights, particles, LFO/audio, post-effects and a grouped subgraph.
- Expectations: save/reopen, undo/redo, bypass, branch, modify node while playing, no disappearing geometry or permanently black viewport after a recoverable error.

8. Testing matrix
Unit: Graph validation, type coercion, cycle rejection, topological ordering, dirty propagation, deterministic random seeds, serialization/migration, graph diff commands and undo semantics.

Integration: Model import with hierarchy/textures, shader uniforms, render resource lifetime, simulation buffer update, Bevy entity reuse, resize, GPU device unavailable/device lost, asset failures, project reload.

Visual: Golden-image test scene screenshots, exposure/tonemapping checks, PBR normals with deformer applied, expected clone distribution, particle spawn/decay. Use thresholded comparisons and manually review intentional renderer changes.

Performance: Browser, OS, GPU/driver, screen resolution, DPR and build profile recorded for every benchmark. Capture CPU graph evaluation, UI input latency, WebGPU frame time if available, total frame time, draw calls and memory. Compare Svelte Flow 100/300/1000 visible nodes as editor-only stress tests; never send 60Hz performance readings to every DOM node.

Browser: Chrome desktop is a supported gate. Edge Chromium is the next candidate. Safari/WebGPU and iOS are conditional expansion targets with feature probing. Firefox and WebGL2 are not promised for compute-heavy scenes.

9. Key risks, tests and responses
Risk    Why it matters    Response
Bevy render API churn    Advanced GPU plugins can break between engine releases    Pin 0.20.x; isolate graphics adapter; upgrade only with fixture and screenshot suite
Browser WebGPU differences    Shader, compute, format, device and memory limits vary    Chrome-first; capability probes; quality tiers; explicit unsupported-state UI
Too much ECS overhead for clones    Massive entity counts can undermine Notch-like performance    Benchmark auto-instancing; custom GPU instance buffer for high-count paths
WASM / DOM main-thread contention    UI interaction can stutter even if GPU throughput is high    Queue/coalesce graph commands; throttle telemetry; measure input latency; evaluate worker only if justified
Naive field implementation    Procedural noise is not a volumetric fluid solver    Separate VectorField from VoxelField types and progression
Shader compile errors/stalls    Creativity suffers if tweaking breaks all output    Validate/debounce, preserve last working pipeline, show inline compile errors
Accidental 'game editor' architecture    Forces procedural work through object/entity editing    Graph-first document model and schema-first node registry
Too much UI polish before visual quality    A beautiful editor with weak effects loses Notch's appeal    Build quality demo fixtures and preset materials early; validate against inspiring references
Browser file/asset friction    Imported 3D assets may contain external texture references    Asset manifest, import processing, repair UI, bundle exports and local caching
Simulation time semantics    Scrubbing can be non-deterministic or painfully slow    Fixed timestep, reset, seed, later checkpoints; label when fast backward scrubbing is not available

10. Issues/backlog, in dependency order
Foundation
- [ ] ADR-001: Adopt Bevy 0.20.x and WebGPU-first browser contract.
- [ ] ADR-002: Own typed project graph separate from Bevy ECS.
- [ ] Set up Rust workspace, Svelte/Vite app, formatting, linting, CI and pinned versions.
- [ ] Svelte viewport pane loads Bevy WASM canvas, resizes cleanly, and reports device capabilities.
- [ ] Add GLB loading + PBR render + lights + camera controls.
- [ ] Add JS→Rust command bridge; slider updates one visual parameter.
- [ ] Run instancing stress test; inspect draw calls and CPU costs.
- [ ] Add a custom shader/material and one post pass.
- [ ] Add WebGPU compute smoke test and GPU error handling.
Editor/graph
- [ ] Define node schema, typed ports, registry and plugin trait.
- [ ] Define versioned graph JSON, stable IDs, asset manifests and migrations.
- [ ] DAG validation, dirty propagation, cache and stateful node semantics.
- [ ] Implement command queue and typed response/event stream.
- [ ] Svelte Flow graph surface, custom node visuals and palette search.
- [ ] Inspector generated from exported registry metadata.
- [ ] Save/load, undo/redo, copy/paste, bypass, reroute and node preview.
Hero composition A
- [ ] Primitives/GLB geometry.
- [ ] Twist, bend and noise deformers with normals strategy.
- [ ] Grid and radial cloners; apply instance transforms and random/noise effectors.
- [ ] PBR/unlit/emissive material nodes; directional/point/environment lights.
- [ ] Camera, scene collector, render target, bloom and tonemapping.
- [ ] Polished sample graph and benchmark chart/report.
Hero composition B
- [ ] Procedural vector field and curl noise, field sample/force.
- [ ] GPU particle storage and fixed-step update.
- [ ] Emitters, gravity, drag, turbulence; sprite renderer and material integration.
- [ ] Time/LFO, keyframes and browser-permission-aware audio FFT.
- [ ] Reset/reseed, diagnostics, sample graph and benchmark report.
Quality and shipping
- [ ] Preset library, compound nodes, thumbnail generation policy.
- [ ] Color grading, feedback and optional DOF; device capability tiering.
- [ ] 2D voxel fluid proof (separate from procedural field).
- [ ] Asset bundle import/export, visual regression and project migrations.
- [ ] Static production deployment and optional native Mac packaging investigation.

11. Explicit stop conditions and trade-off decisions
- Do not start with five nodes: implement enough cross-category operations to make two complete visual compositions. Operator count is secondary to capability coverage and joy of use.
- Do not implement advanced 3D voxel fluids before a stable particle/field path: reuse the scheduling and resource model first.
- Do not write a custom node editor from scratch yet: ship with Svelte Flow and benchmark real projects; change only if proved inadequate.
- Do not put every clone/particle into the Bevy ECS: use resource handles and GPU-centric buffers for high cardinality.
- Do not require a native companion app: browser editing is the first product. Native is a reuse/packaging question.
- Do not build an entire TouchDesigner-equivalent data language: direct modulation, typed arrays/signals and an extensible node registry are sufficient foundations.
- Do not let the UI define renderer architecture: graph-core and renderer bridge must compile/test without Svelte.

12. Kickoff instruction for a coding agent
Create a new Rust workspace and Svelte 5/Vite application for a browser-first node-based real-time 3D graphics engine. Use Bevy 0.20.x with WASM/WebGPU, custom WGSL/WESL shaders, and Svelte Flow. Begin only with Gate A. Produce an interactive browser scene with GLB loading, PBR, lighting, camera orbit, a Svelte slider controlling a Rust-side parameter, verified instancing, one custom post-process pass, and a minimal compute shader. Provide repeatable build scripts, hardware/browser capability reporting, and a short written spike report measuring performance and integration risks. Keep app-specific creative node graph logic out of Bevy ECS until Gate B. Do not pretend WebGL2 is a fallback for compute-driven effects. Avoid React. Pin dependencies and preserve a path for native builds. After Gate A, implement the graph/editor milestones in the order of this plan.

13. Primary references and live verification
- Bevy WebGPU examples — working WASM+WebGPU 3D examples and source links.
- Bevy 0.20 release, 8 Oct 2026 — version and renderer changes, WESL support.
- Bevy automatic instancing example — first test before custom instancing.
- Bevy custom GPU instancing — advanced rendering path.
- Bevy custom post-processing example — render pipeline integration reference.
- Bevy WebGPU browser feature — webgpu and webgl2 are distinct build targets, not automatic fallback.
- Svelte Flow docs — customizable Svelte node editor and graph interactions.
- Notch fields — separate field simulation and affectors.
- Notch deformers — geometry operations benchmark.
- Notch cloning — high-count instanced geometry and effectors.
- Notch particles — emitter/affector/renderer separation.
- WebGPU browser status — platform coverage and limitations; verify on deployment target.
Source reliability note: These links substantiate framework features and precedent. Performance goals, node API, file layout and milestone gates are proposed engineering decisions and must be verified by building Gate A.
