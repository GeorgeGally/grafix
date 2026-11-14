# grafix

Browser-first, node-based real-time 3D graphics engine. Rust + Bevy 0.20 on
WebGPU, a Svelte 5 interface, custom WGSL/WESL shaders. See
[docs/PLAN.md](docs/PLAN.md) for the full plan.

**Status: Gate A passed; Gate B typed graph and Svelte Flow editor implemented.** Gate A renderer is the graphics backend for the first graph nodes. See [Gate A](docs/decisions/gate-a-spike.md) and [Gate B](docs/decisions/gate-b-implementation.md).

## Requirements

- [rustup](https://rustup.rs). `rust-toolchain.toml` pins Rust 1.99.1 and the
  `wasm32-unknown-unknown` target; rustup installs them on first build.
- Node 22 or newer.
- A WebGPU browser: Chrome 113+ on desktop. There is no WebGL fallback.

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
results appear in the Diagnostics panel and on `window.__grafixBench`.

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
