//! Shader sources for the engine.
//!
//! Every shader is embedded into the binary with [`embedded_asset!`] so the
//! browser build never fetches shader files over HTTP and the native build
//! needs no asset directory for them.

use bevy::{asset::embedded_asset, prelude::*};

pub const POST_PROCESS: &str = "embedded://graphics_shaders/shaders/post_process.wesl";
pub const CUSTOM_INSTANCING: &str = "embedded://graphics_shaders/shaders/custom_instancing.wesl";
pub const COMPUTE_FIELD: &str = "embedded://graphics_shaders/shaders/compute_field.wesl";
pub const GRID_FLOOR: &str = "embedded://graphics_shaders/shaders/grid_floor.wesl";

pub struct GraphicsShadersPlugin;

impl Plugin for GraphicsShadersPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/post_process.wesl");
        embedded_asset!(app, "shaders/custom_instancing.wesl");
        embedded_asset!(app, "shaders/compute_field.wesl");
        embedded_asset!(app, "shaders/grid_floor.wesl");
    }
}
