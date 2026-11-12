# Browser support

| Browser | Status |
| --- | --- |
| Chrome desktop 113+ | Supported; Gate A was verified on Chrome for Testing 156 (macOS, Metal) |
| Edge (Chromium) | Next candidate; not yet tested |
| Safari / iOS WebGPU | Conditional; probe only |
| Firefox | Not promised |
| Any browser without WebGPU | Unsupported. The app shows a message and does not load the engine |

The engine is built with Bevy's `webgpu` feature only. WebGL2 is not a
fallback: compute passes, storage textures and GPU-side instancing need WebGPU.

## Capability report

At startup, the page probes WebGPU from JavaScript (adapter info, features,
limits, preferred canvas format, DPR, cores, memory) before it loads any wasm.
Once the renderer starts, the engine adds what wgpu actually negotiated
(backend, features, limits, GPU preprocessing level). **Copy report** in the
Device panel copies both as JSON. Reference reports live in
`tests/benchmarks/`.

## Errors

- No WebGPU, or no adapter: a message in the viewport, and the engine is never
  downloaded.
- Device lost: the error is shown, and the renderer re-initialises once on a
  new device. A second loss stops rendering and offers Reload.
- Validation, out-of-memory or internal GPU errors: rendering stops (instead of
  flashing) and an overlay shows the error with Reload.
- Shader/pipeline compile errors and asset load failures appear in the
  viewport's error list.
