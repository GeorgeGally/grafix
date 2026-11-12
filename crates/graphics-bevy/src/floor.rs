//! Custom WGSL/WESL material test: a StandardMaterial extension that draws a
//! procedural grid while keeping Bevy's PBR lighting and shadows.

use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

pub type GridFloorMaterial = ExtendedMaterial<StandardMaterial, GridFloor>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct GridFloor {
    #[uniform(100)]
    pub line_color: LinearRgba,
    #[uniform(100)]
    pub cell_size: f32,
    #[uniform(100)]
    pub line_width: f32,
    #[uniform(100)]
    pub _pad0: f32,
    #[uniform(100)]
    pub _pad1: f32,
}

impl Default for GridFloor {
    fn default() -> Self {
        Self {
            line_color: LinearRgba::rgb(0.09, 0.11, 0.14),
            cell_size: 0.5,
            line_width: 1.0,
            _pad0: 0.0,
            _pad1: 0.0,
        }
    }
}

impl MaterialExtension for GridFloor {
    fn fragment_shader() -> ShaderRef {
        graphics_shaders::GRID_FLOOR.into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        graphics_shaders::GRID_FLOOR.into()
    }
}

pub(crate) struct GridFloorPlugin;

impl Plugin for GridFloorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<GridFloorMaterial>::default());
    }
}
