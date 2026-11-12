//! Assembles the Bevy app used by both the browser bridge and the native CLI.
//!
//! This crate owns no creative node-graph logic: it exposes a small typed
//! command queue and a telemetry snapshot. The graph (Gate B) will drive the
//! engine through the same command boundary.

use bevy::{asset::AssetMetaCheck, prelude::*, window::WindowResolution};

pub mod commands;
pub mod floor;
pub mod instancing;
pub mod orbit;
pub mod scene;
pub mod telemetry;

pub use commands::{CommandQueue, EngineCommand, EngineParams, ParamKey};
pub use instancing::InstancingMode;
pub use telemetry::{EngineTelemetry, RenderCapabilities, TelemetrySnapshot};

/// How the host wants the engine window set up.
#[derive(Clone, Debug, Default)]
pub struct EngineConfig {
    /// CSS selector of an existing `<canvas>` (web only).
    pub canvas: Option<String>,
    /// Asset root: a URL path on the web, a directory natively.
    pub asset_root: String,
    /// Initial window size for native runs.
    pub window_size: Option<(u32, u32)>,
}

/// Builds the app without running it. The caller keeps the command queue and
/// telemetry handles to talk to the engine.
pub fn build_app(config: &EngineConfig) -> (App, CommandQueue, EngineTelemetry) {
    let mut window = Window {
        title: "grafix".into(),
        canvas: config.canvas.clone(),
        fit_canvas_to_parent: true,
        prevent_default_event_handling: true,
        ..default()
    };
    if let Some((w, h)) = config.window_size {
        window.resolution = WindowResolution::new(w, h);
    }

    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.015, 0.017, 0.022)))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(window),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: config.asset_root.clone(),
                    // The browser build has no .meta files; probing for them
                    // would put 404s in the console.
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                }),
        );

    let queue = CommandQueue::default();
    let telemetry = EngineTelemetry::default();
    app.add_plugins((
        graphics_gpu::GraphicsGpuPlugin,
        floor::GridFloorPlugin,
        commands::CommandsPlugin {
            queue: queue.clone(),
        },
        telemetry::TelemetryPlugin {
            telemetry: telemetry.clone(),
        },
        scene::ScenePlugin,
        orbit::OrbitCameraPlugin,
        instancing::InstancingPlugin,
    ));
    (app, queue, telemetry)
}
