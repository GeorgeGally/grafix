// Custom WGSL/WESL material: extends StandardMaterial with procedural grid
// lines, keeping Bevy's PBR lighting.
import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::{
    prepass::io::{VertexOutput, FragmentOutput},
    deferred::functions::deferred_output,
};
@else
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

struct GridFloor {
    line_color: vec4<f32>,
    cell_size: f32,
    line_width: f32,
    _pad0: f32,
    _pad1: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> grid: GridFloor;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let coord = in.world_position.xz / grid.cell_size;
    let derivative = fwidth(coord);
    let cell = abs(fract(coord - 0.5) - 0.5) / derivative;
    let line = 1.0 - min(min(cell.x, cell.y) / grid.line_width, 1.0);
    let fade = 1.0 - smoothstep(4.0, 14.0, length(in.world_position.xz));
    pbr_input.material.base_color = mix(pbr_input.material.base_color, grid.line_color, line * fade);

    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color
    );

    var out: FragmentOutput;
@if(PREPASS_PIPELINE) {
    out = deferred_output(in, pbr_input);
} @else {
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
}
    return out;
}
