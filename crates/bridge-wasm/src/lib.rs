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
        HANDLES.with(|h| *h.borrow_mut() = Some(Handles { queue, telemetry }));
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
