# R0 — Graph owns the scene: implementation & acceptance

**Date:** 21 November 2026  
**Branch:** `feat/typed-graph-editor`  
**Scope:** The first trust/correctness slice of the [Notch gap audit](../plans/2026-11-21-notch-functional-ux-audit.md).

## What changed

1. **A single creative control surface.** The old Scene and Clones sliders are removed from the primary UI. The diagnostics sidebar is hidden by default and opened with **Debug**. Benchmark commands may temporarily control the renderer but restore the last validated graph on completion; they never rewrite the saved composition.
2. **The source material really reaches clones.** `graph-core::Controls.clone_material` is computed from the material connected to the geometry wired to the active Fibonacci Cloner. `GraphCloneAppearance` carries it through Bevy's command queue. Built-in and unbatched clones use `StandardMaterial`; fast custom GPU clones use the linked base colour, metallic/roughness response and emissive RGB in their custom WGSL shader.
3. **Visible execution status from Rust.** `execution_status` returns per-node live, disconnected, incomplete, partial or unsupported status. The Svelte graph renders those states directly on the node and in the inspector. A material connected to custom GPU clones is explicitly labelled as an **approximate PBR** surface; the custom shader does not inherit the full Bevy PBR lighting/shadow pipeline.
4. **Earlier connection errors.** The editor rejects self-connections, duplicate connections, incompatible port types and introduced cycles before committing a graph transaction. Rust remains authoritative for full graph validation and safety.
5. **More reliable undo.** Continuous colour picker and keyboard slider edits are grouped into gestures, rather than adding one history entry for every intermediate colour sample.
6. **Integration coverage.** Rust graph tests assert material propagation/disconnection and execution status; a Bevy ECS test asserts that applying a compiled graph updates the live scene and cloner material resources. Browser tests exercise status labels, graph editing and persistence (with a mocked bridge).

## Compatibility

- No changes to `format_version: 1` or persisted node identifiers.
- Existing projects with an explicit Material → Cube → Cloner connection now **correctly change the clones' appearance**. This may change the look of previous compositions that depended on the old hardcoded cloner gradient, but makes connection semantics truthful.
- An unconnected material leaves the legacy clone colour palette intact.
- The Gate A benchmark functions and diagnostics are preserved; their controls no longer compete with authoring UI.

## Manual Chrome/WebGPU acceptance checklist

1. Build and run using `scripts/build-web.sh && scripts/serve.sh`; hard refresh Chrome.
2. In the original composition, select the PBR Material wired to the Cube → Fibonacci Cloner. Change base colour red/blue and roughness/metallic. **Actual clones, not only the standalone cube, must visibly respond.**
3. Switch the cloner between **custom** and **builtin**. All modes must respect source colour. The fast custom GPU path is explicitly described as *approximate* compared with Bevy PBR.
4. Disconnect material from cube; the legacy clone palette should return and the material should become disconnected unless still used by another live object.
5. Add a detached node: the node and inspector must show **disconnected**. Connect it into the scene: state changes appropriately.
6. Press **Debug**, run the benchmark, close Debug. Verify graph-clone count, scene settings, undo history and saved JSON did not change.
7. Save/reload and verify two independent materialized cubes still render in the Two objects preset.
8. Repeat through a GPU device loss/recovery and capture renderer errors/capabilities.

## Not yet complete

- **Real rendered image tests are still outstanding.** Existing Playwright browser tests use a mocked bridge. ECS state tests and compiler checks are not proof of correct GPU pixels; a real macOS/Chrome visual pass is required.
- No arbitrary imported GLB assets, object picking/raycast gizmos, generalized graph evaluator or GPU-only group transform uniforms yet.
- The startup Bevy scene still owns the demonstration floor and some hidden proof-of-concept entities. Replacing these with graph-authoritative environment and viewport helpers is a separate R0 follow-on.
- The fast GPU clone shader uses analytic surface lighting rather than shadowed Bevy PBR. Avoid claiming the two paths are visually identical.
- Graph transaction ownership currently lives in `GraphEditor` rather than a standalone shared `ProjectState` store. A dedicated store becomes necessary when a full asset browser and timeline begin editing the same project.

**Next technical gate:** automated real-WASM/WebGPU scene smoke tests and removal of remaining demo-owned geometry before starting R1/R2.
