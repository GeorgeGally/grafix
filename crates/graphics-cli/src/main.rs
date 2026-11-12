//! Native host for the engine. Runs the same Bevy app as the browser build in
//! a desktop window, reading assets from the workspace `assets/` directory.
//!
//! `graphics-cli [--instancing <mode> <count>]`

use graphics_bevy::{EngineCommand, EngineConfig, InstancingMode};

fn main() {
    let asset_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").to_string();
    let (mut app, queue, _telemetry) = graphics_bevy::build_app(&EngineConfig {
        canvas: None,
        asset_root,
        window_size: Some((1280, 720)),
    });

    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [flag, mode, count, ..] = args.as_slice()
        && flag == "--instancing"
    {
        let mode = InstancingMode::parse(mode).expect("mode: off|builtin|unbatched|custom");
        let count = count.parse().expect("count must be an integer");
        queue.push(EngineCommand::SetInstancing { mode, count });
    }
    app.run();
}
