# ADR-001: Bevy 0.20 on WebGPU as the browser contract

Status: accepted for Gate A (12 November 2026)

## Decision

- Render with Bevy `=0.20.1` (wgpu 30) compiled to `wasm32-unknown-unknown`
  with the `webgpu` feature. The `webgl2` feature is not enabled anywhere in
  the workspace, so there is no silent WebGL2 path: compute-driven effects
  need WebGPU and the app says so when it is missing.
- Chrome desktop is the supported browser. Others are probed, not promised.
- Bevy runs on the page's main thread and draws into a `<canvas>` owned by the
  Svelte viewport pane (`Window::canvas` + `fit_canvas_to_parent`).
- The JS side talks to Rust only through `bridge-wasm`: typed commands into a
  queue the engine drains once per frame in `PreUpdate`, and a telemetry
  snapshot JS polls at 4 Hz. No GPU data and no per-frame events cross the
  boundary.
- Custom GPU work uses public Bevy extension points only: the `Core3d` render
  schedule (post-process), the root `RenderGraph` schedule (compute, ordered
  before `camera_driver`), `SpecializedMeshPipeline` + a render command
  (custom instancing), and `MaterialExtension` (custom material).
- Render errors go through Bevy's `RenderErrorHandler`: errors are recorded in
  telemetry, device loss gets one automatic re-initialisation attempt, other
  errors stop rendering (no strobing, no `AppExit`) and the UI offers Reload.
- The same app assembly (`graphics_bevy::build_app`) runs natively through
  `graphics-cli`, which keeps the native path compiling.

## Consequences

- Shaders are embedded with `embedded_asset!`, so the browser fetches only the
  wasm, JS and model files.
- Bevy 0.20 replaced the render graph with schedules; any upgrade must re-run
  the Gate A checks (post pass ordering after `tonemapping`, compute before
  `camera_driver`, custom instancing phase code).
- The release wasm is large (see the spike report); size and startup time are
  tracked risks.
