# grafix

Browser-first, node-based real-time 3D graphics engine. Rust + Bevy 0.20 on
WebGPU, a Svelte 5 interface, custom WGSL/WESL shaders. See
[docs/PLAN.md](docs/PLAN.md) for the full plan.

**Status: Gate A passed; Gate B implemented; Gate C0/C1 multi-object node foundations implemented on `feat/typed-graph-editor` (visual acceptance pending).** Gate A renderer is the graphics backend for the first graph nodes. See [Gate A](docs/decisions/gate-a-spike.md) and [Gate B](docs/decisions/gate-b-implementation.md).

## Requirements

- [rustup](https://rustup.rs). `rust-toolchain.toml` pins Rust 1.99.1 and the
  `wasm32-unknown-unknown` target; rustup installs them on first build.
- Node 22 or newer.
- A WebGPU browser: Chrome 113+ on desktop. There is no WebGL fallback.

## First multi-object composition (Gate C0/C1)

Run the editor, click **Two objects** in the graph toolbar, and select the red
and blue Cube or PBR Material nodes. Changes to transform, base colour,
metallic, roughness and visibility are graph-owned, independent and persistent.
The bundled [two-material-cubes preset](assets/presets/two-material-cubes.grafix.json)
is also importable with **Open**. Multiple directional lights can be connected
to the Scene.light input. Disconnected objects and lights are removed.

**R0 authoring trust:** Creative controls now live in the graph only; open
**Debug** for read-only telemetry and a benchmark that restores the graph.
PBR materials connected to the Cube source of Fibonacci Cloner now affect
clones in native and custom GPU modes (the fast custom shader uses approximate,
not full, PBR lighting). Per-node evaluation badges explain disconnected or
partially supported paths. See [R0 implementation and visual checklist](docs/decisions/r0-graph-owns-scene.md).

**Not yet shipped:** arbitrary GLB file import, PBR material override for glTF,
primitive variety, deformers, object raycast gizmos, GPU-only uniform transforms
and full Gate C. The browser CI uses a mocked renderer for UI tests; real
macOS/Chrome WebGPU imagery and performance still need acceptance.

## Build and run

```sh
scripts/build-web.sh   # release wasm -> wasm-bindgen -> Vite production bundle
scripts/serve.sh       # http://127.0.0.1:4173/
```

`scripts/build-web.sh` installs `wasm-bindgen-cli` into `.tools/` at the exact
version `Cargo.lock` pins, so the bindings always match the crate. Development
loop with hot reload of the UI:

```sh
scripts/build-web.sh --wasm          # rebuild after Rust changes (add --dev for faster compiles)
(cd apps/editor && npm run dev)      # http://127.0.0.1:5173/
```

Native build (same engine, desktop window, reads `assets/` from disk):

```sh
cargo run --release -p graphics-cli -- --instancing custom 10000
```

Static checks: `scripts/check.sh` (rustfmt, clippy for wasm and native, svelte-check). Run `(cd apps/editor && npm install)` to update the package lock when checking out the new editor dependencies. Graph tests: `cargo test --locked -p graph-core`.

Append `?bench` to the URL to run the instancing benchmark sweep on load;
results appear in the **Debug** diagnostics panel and on `window.__grafixBench`.

## Layout

| Path | Role |
| --- | --- |
| `crates/graph-core` | Typed graph documents, node registry, validator, compiler and tests |
| `crates/graphics-shaders` | WGSL/WESL sources, embedded into the binary |
| `crates/graphics-gpu` | Render-world extensions: post-process pass, custom instancing, compute pass |
| `crates/graphics-bevy` | App assembly: scene, PBR, lights, orbit camera, clone modes, command queue, telemetry, render error handling |
| `crates/bridge-wasm` | JS/WASM API (`start`, `set_param`, `set_instancing`, `telemetry`, ...) |
| `crates/graphics-cli` | Native host for the same app |
| `apps/editor` | Svelte Flow patch workspace, generated inspector, viewport, diagnostics, benchmarks and JSON projects |
| `assets/demo` | License-safe demo assets (see `assets/demo/LICENSE.md`) |
| `tests/benchmarks` | Recorded capability reports and benchmark results |

Dependencies are pinned: Bevy `=0.20.1`, `Cargo.lock`, exact npm versions with
`package-lock.json`.
