//! Browser entry point. JS calls `start(canvasSelector, assetRoot)` once, then
//! talks to the running engine through `set_param`, `set_instancing`,
//! `set_animate_clones`, `reset_camera` and `telemetry`.
//!
//! Commands are queued and applied by the engine at a frame boundary; nothing
//! here touches the ECS directly, and no GPU data crosses the boundary.

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;

    use graphics_bevy::{
        CommandQueue, EngineCommand, EngineConfig, EngineTelemetry, InstancingMode, ParamKey,
    };
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(inline_js = "export function report_fatal(message) {
        window.dispatchEvent(new CustomEvent('grafix:fatal', { detail: message }));
    }")]
    extern "C" {
        fn report_fatal(message: &str);
    }

    struct Handles {
        queue: CommandQueue,
        telemetry: EngineTelemetry,
        graph: RefCell<graph_core::Document>,
    }

    thread_local! {
        static HANDLES: RefCell<Option<Handles>> = const { RefCell::new(None) };
    }

    fn with_handles<R>(f: impl FnOnce(&Handles) -> R) -> Result<R, JsValue> {
        HANDLES.with(|h| {
            h.borrow()
                .as_ref()
                .map(f)
                .ok_or_else(|| JsValue::from_str("engine not started"))
        })
    }

    /// Starts the engine on an existing canvas. Returns once the app is
    /// scheduled; Bevy then runs from the browser's animation frames.
    #[wasm_bindgen]
    pub fn start(canvas_selector: String, asset_root: String) -> Result<(), JsValue> {
        if HANDLES.with(|h| h.borrow().is_some()) {
            return Err(JsValue::from_str("engine already started"));
        }
        std::panic::set_hook(Box::new(|info| {
            console_error_panic_hook::hook(info);
            report_fatal(&info.to_string());
        }));
        let (mut app, queue, telemetry) = graphics_bevy::build_app(&EngineConfig {
            canvas: Some(canvas_selector),
            asset_root,
            window_size: None,
        });
        HANDLES.with(|h| *h.borrow_mut() = Some(Handles {
            queue,
            telemetry,
            graph: RefCell::new(graph_core::Document::demo()),
        }));
        app.run();
        Ok(())
    }

    /// Sets one live parameter. Unknown names are an error, not a no-op.
    #[wasm_bindgen]
    pub fn set_param(name: &str, value: f64) -> Result<(), JsValue> {
        let key = ParamKey::parse(name)
            .ok_or_else(|| JsValue::from_str(&format!("unknown parameter: {name}")))?;
        with_handles(|h| {
            h.queue.push(EngineCommand::SetParam {
                key,
                value: value as f32,
            })
        })
    }

    /// `mode` is one of `off`, `builtin`, `unbatched`, `custom`.
    #[wasm_bindgen]
    pub fn set_instancing(mode: &str, count: u32) -> Result<(), JsValue> {
        let mode = InstancingMode::parse(mode)
            .ok_or_else(|| JsValue::from_str(&format!("unknown instancing mode: {mode}")))?;
        with_handles(|h| h.queue.push(EngineCommand::SetInstancing { mode, count }))
    }

    #[wasm_bindgen]
    pub fn set_animate_clones(animate: bool) -> Result<(), JsValue> {
        with_handles(|h| h.queue.push(EngineCommand::SetAnimateClones(animate)))
    }

    #[wasm_bindgen]
    pub fn reset_camera() -> Result<(), JsValue> {
        with_handles(|h| h.queue.push(EngineCommand::ResetCamera))
    }

    /// Latest telemetry snapshot as JSON. Poll at a low rate.
    #[wasm_bindgen]
    pub fn telemetry() -> Result<String, JsValue> {
        with_handles(|h| h.telemetry.snapshot_json())
    }

    /// Rust-owned typed node registry for the Svelte Flow palette and inspector.
    #[wasm_bindgen]
    pub fn graph_schema() -> String {
        serde_json::to_string(&graph_core::registry()).unwrap_or_else(|_| "[]".into())
    }

    /// The default composition reproduces the Gate A scene, but as a graph.
    #[wasm_bindgen]
    pub fn default_graph() -> String {
        serde_json::to_string(&graph_core::Document::demo()).unwrap_or_default()
    }

    /// Validate the entire graph before accepting it. Connection changes are
    /// atomic: invalid edits cannot replace the last working scene.
    #[wasm_bindgen]
    pub fn load_graph(json: &str) -> Result<(), JsValue> {
        let document: graph_core::Document = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let projection = graph_core::compile(&document)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        with_handles(|h| {
            h.graph.replace(document);
            h.queue.push(EngineCommand::ApplyGraph(projection));
        })
    }

    /// One-parameter command for continuous slider edits. The graph remains
    /// Rust-validated; no entire JSON document crosses the bridge per change.
    #[wasm_bindgen]
    pub fn set_node_param(node_id: &str, key: &str, value: f64) -> Result<(), JsValue> {
        if !value.is_finite() { return Err(JsValue::from_str("non-finite parameter")); }
        with_handles(|h| {
            let mut candidate = h.graph.borrow().clone();
            let node = candidate.nodes.iter_mut().find(|n| n.id == node_id)
                .ok_or_else(|| JsValue::from_str("node does not exist"))?;
            node.params.insert(key.to_owned(), serde_json::json!(value));
            let projection = graph_core::compile(&candidate)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            h.graph.replace(candidate);
            h.queue.push(EngineCommand::ApplyGraph(projection));
            Ok::<(), JsValue>(())
        })?
    }

    /// Parameter names and ranges, so the UI is generated from Rust's list.
    #[wasm_bindgen]
    pub fn param_schema() -> String {
        let entries: Vec<String> = ParamKey::ALL
            .iter()
            .map(|key| {
                let (min, max) = key.range();
                format!(r#"{{"name":"{}","min":{min},"max":{max}}}"#, key.name())
            })
            .collect();
        format!("[{}]", entries.join(","))
    }
}
