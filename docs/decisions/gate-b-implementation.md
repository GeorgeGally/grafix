# Gate B: Typed graph and live WebGPU editor

**Branch:** feat/typed-graph-editor

## Implemented
- Gate A remains intact.
- `graph-core` is a Bevy-free, versioned Rust document, type checker, registry,
  topological sorter and live-control compiler with unit tests.
- Bevy commands accept an atomically validated graph projection at a frame
  boundary. Live connections drive visibility, GPU-instancing count/mode,
  PBR lighting, post-effects and a time-driven bloom LFO.
- `bridge-wasm` exports graph schema, demo graph, validated load and individual
  numeric parameter edits. The editor sends slider updates without resending
  full graph JSON every frame.
- The Svelte Flow editor has typed ports, searchable palette, schema-driven
  inspector, JSON save/open, local persistence, node repositioning and undo/redo.

## Scope boundary

This is the **Gate B implementation**, not Gates C-F. The cloner still
instances the Gate A cube using its Fibonacci spherical layout; glTF loading
is the bundled BoomBox. The 2D field node is a compute-backed texture,
**not** voxel-fluid simulation. Advanced deformers, procedural geometry,
GPU particle systems, generic signals, scene grouping and export are next.

## Tests and integration checks

- `cargo test --locked -p graph-core`
- `cargo check --locked -p graphics-cli`
- `cargo check --locked --target wasm32-unknown-unknown -p bridge-wasm`
- `(cd apps/editor && npm install && npm run check)` after WASM bindings build.
- `scripts/build-web.sh` for the complete browser bundle.
- Browser manual test: reconnect model/cloner/field, adjust node params,
  add LFO -> bloom, undo, redo, save/reopen JSON, reload from localStorage,
  and rerun Gate A benchmark.
- Browser runtime confirmation pending CI/build execution; no claim of
  real-machine GPU test in this branch.

## Follow-up technical debt
1. Real general geometry/instance/material handles instead of Gate A adapters.
2. Generic typed dataflow evaluator to replace the one-LFO specialisation.
3. Project migration and asset repair.
4. More undoable graph gestures, previews, compound nodes and selection.
5. Particle/field compute lifetimes and time/reset semantics.
