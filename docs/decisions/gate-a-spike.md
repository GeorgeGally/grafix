# Gate A spike report: Bevy 0.20 on WebGPU, built-in vs custom instancing

Date: 12 November 2026. Verdict: **pass, continue with Bevy.** No Bevy
internals were patched; every extension uses public 0.20 APIs. One recovery
gap in Bevy needed a small workaround (risk 2 below).

**Reference machine:** MacBook Pro, Apple M3 Max, macOS 26.3.1. **Browser:**
Chrome for Testing 156.0.8078.4, headless, WebGPU on Metal (`--enable-unsafe-webgpu
--enable-gpu --ignore-gpu-blocklist`). Canvas 1300×900 at DPR 1. Release
build. Capability report: `tests/benchmarks/gate-a/capabilities-m3max-chrome156.json`.
Raw results: `tests/benchmarks/gate-a/instancing-m3max-chrome156.json`.

## Pass gate

| Check | Result |
| --- | --- |
| GLB renders | BoomBox.glb (CC0) with PBR textures, directional light with 2-cascade shadows, two point lights, bloom, TonyMcMapface tonemapping |
| Live parameters, no restart | 7 Rust-owned parameters driven from Svelte sliders through the command queue. After a slider sweep: renderer inits still 1, pipeline count unchanged (39) |
| Compute pass | 256×256 storage-texture compute shader runs every frame (dispatch counter rising) and is shown as an emissive panel |
| Resizing | Canvas buffer matched layout at 600×700, 1300×900, 1620×1080 and 2100×1300; with a real DPR of 2 the buffer was 1960×1600 for a 980×800 box. 60 fps, no errors throughout |
| Clean console | Only Bevy `INFO` lines. No warnings or errors on load, slider use, mode switches or resizes |
| GPU/device errors visible | Device loss: the error appears in the viewport banner, the renderer re-initialises itself, then a "recovered" notice shows (instancing, compute and post all come back). A second loss, or any validation error, stops rendering and shows an overlay with Reload. No WebGPU: a message and no wasm download. |

Not checked: real Chrome stable (157) in a visible window, and the native
binary (it passes clippy for the host target but was not run, because the
spike stays headless).

## Built-in vs custom instancing

The same clone layout drawn three ways (`crates/graphics-bevy/src/instancing.rs`).
Draws, instances and upload bytes were counted by wrapping the WebGPU API in
the page, so they don't depend on what Bevy reports about itself. Frame time
is capped by vsync at 16.67 ms, so CPU time is the figure to compare. A
baseline of 30 draws and 2.25 ms CPU is the scene with no clones.

| Strategy | Clones | Frame ms (p95) | CPU ms | Draws/frame | Upload KB/frame | ECS entities |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Built-in, CPU-animated | 1,000 | 16.7 (16.7) | 3.0 | 33 | 227 | 1,301 |
| Built-in, CPU-animated | 10,000 | 16.7 (16.8) | 10.6 | 33 | 2,261 | 10,301 |
| Built-in, CPU-animated | 50,000 | **52.9** (66.7) | **52.1** | 33 | 11,946 | 50,301 |
| Built-in, static | 10,000 | 16.7 (16.8) | 5.7 | 33 | 245 | 10,301 |
| Built-in, static | 50,000 | 17.2 (16.8) | 17.4 | 33 | 1,160 | 50,301 |
| Unbatched (unique materials) | 1,000 | 16.7 (16.8) | 10.8 | 2,960 | 234 | 1,301 |
| Unbatched (unique materials) | 10,000 | **71.8** (83.4) | **72.0** | 29,183 | 9,150 | 10,301 |
| Custom, GPU-animated | 1,000 | 16.7 (16.7) | 2.4 | 31 | 16 | 302 |
| Custom, GPU-animated | 10,000 | 16.7 (16.7) | 2.6 | 31 | 16 | 302 |
| Custom, GPU-animated | 50,000 | 16.7 (16.8) | 2.3 | 31 | 16 | 302 |
| Custom, GPU-animated | 100,000 | 16.7 (16.8) | 2.8 | 31 | 16 | 302 |

Results from a single run.

- **Built-in instancing works.** Clones that share a mesh and material collapse
  into one draw per pass: +3 draws (main pass plus two shadow cascades) at any
  count. Breaking batching costs one draw per clone per pass. At 10k clones
  that is 29k draws and 72 ms.
- **The limit for built-in is the CPU, not the GPU.** Every clone is an ECS
  entity: CPU time grows from 3 ms to 52 ms between 1k and 50k animated clones,
  and Bevy re-uploads transforms every frame (about 240 bytes per animated clone
  per frame, and still about 23 bytes per clone when they're static).
  On this machine built-in holds 60 fps up to roughly 10–20k animated clones.
- **Custom instancing is flat.** One entity and one draw call. Instance data is
  uploaded once and then only when it changes, and the animation runs in the
  vertex shader. CPU time stays at the no-clone baseline up to 100k. This
  matches the plan: high-count clones and particles belong in GPU buffers, not
  in ECS.
- **The comparison is not like-for-like:** in this spike the custom path has no
  PBR lighting, no shadows and no frustum culling, and it needs
  `NoIndirectDrawing` on the camera. Gate C has to add lighting (the PBR
  functions plus `mesh_view_bindings`), shadow-pass queuing, and
  compute-written instance buffers before it can replace built-in instancing.

## Other measurements

- Load: wasm 46.3 MB raw / 10.1 MB gzip (with the 44 MB `name` section stripped).
  From navigation to the model loaded and rendering: 2.5 s from a local server
  (wasm fetched in 44 ms, GLB in 190 ms). 39 GPU pipelines created on startup.
  JS heap 43 MB.
- GPU preprocessing reports `preprocessing_only` on WebGPU: no GPU culling, and
  nearly all draws are direct (2 indirect draws per frame).
- Compile: clean release wasm build in 2 min 36 s. An incremental rebuild of
  the engine crates takes about 8 s.
- Not measured: GPU frame time (the adapter exposes `timestamp-query`, but the
  spike does not read it yet), shader compile time per pipeline, GPU memory,
  input latency, Svelte Flow editor performance (Gate B), a discrete-GPU
  machine, and visible-window Chrome stable.

## Integration risks

1. **Render API churn.** In 0.20 the render graph became schedules (`Core3d`,
   `RenderGraph`), shaders are WESL, and the custom instancing code depends on
   `MeshPipeline` internals (`ViewKeyCache`, `RenderMeshInstances`,
   `SetMeshViewBindingArrayBindGroup`). Pin `=0.20.1`. Any upgrade must re-run
   these checks.
2. **Device-loss recovery is incomplete in Bevy 0.20.** After
   `RenderErrorPolicy::Recover`, the window's `SurfaceData` still belonged to
   the lost device, so every frame failed in the `upscaling` pass. Fixed by
   removing `SurfaceData` on `RenderStartup` (`graphics-bevy/src/telemetry.rs`).
   Our own render-world caches have to be reset at the same point: the instance
   buffers, and `SpecializedMeshPipelines`, whose IDs otherwise pointed into
   the new `PipelineCache` at the wrong pipelines. Report this upstream; every
   custom render-world resource needs a reset on `RenderStartup`.
3. **Binary size.** 10 MB gzip before `wasm-opt`, which is not part of the
   pinned toolchain yet. Startup over a real network will be slower than the
   local figure.
4. **Main-thread contention.** Bevy, the Svelte UI and telemetry polling share
   one thread. The UI reads telemetry at 4 Hz, and no per-frame data reaches the
   DOM. Input latency is still unmeasured; under heavy CPU-instanced load,
   slider input would stall along with the frame.
5. **Browser privacy reductions.** WebGPU hides the adapter name, vendor ID and
   driver (only `apple` / `metal-3` here), so per-GPU quality tiers have to rely
   on limits and features, not names.
6. **Test environment.** These numbers come from headless Chrome for Testing,
   whose CDP DPR emulation does not scale the device-pixel box (a real
   `--force-device-scale-factor=2` launch was used instead). Confirm against
   visible-window Chrome stable before quoting results externally.
7. **Build-time advisory.** `npm audit` reports a low-severity `deepmerge`
   advisory in `@sveltejs/vite-plugin-svelte` with no fix available. It is
   build-time only.

## Not in Gate A

The HDRI/environment map is deferred (ambient light and point lights stand in
for it), because the only ready-made KTX2 maps are Bevy's own demo assets, whose
licence hasn't been checked. Svelte Flow is not installed yet: it arrives with
the graph editor in Gate B. The `graph-*` crates do not exist, by design (see
ADR-002).
