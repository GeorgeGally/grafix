// Custom fullscreen post-process pass: chromatic aberration and vignette.
// Runs after Bevy's tonemapping, on the camera's HDR main texture.
import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct PostProcessSettings {
    aberration: f32,
    vignette: f32,
    _pad0: f32,
    _pad1: f32,
}
@group(0) @binding(2) var<uniform> settings: PostProcessSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let centered = in.uv - vec2<f32>(0.5);
    // Aberration grows towards the frame edge.
    let offset = centered * settings.aberration;
    let r = textureSample(screen_texture, texture_sampler, in.uv + offset).r;
    let g = textureSample(screen_texture, texture_sampler, in.uv).g;
    let b = textureSample(screen_texture, texture_sampler, in.uv - offset).b;
    let falloff = 1.0 - settings.vignette * smoothstep(0.2, 0.85, length(centered) * 1.4);
    return vec4<f32>(vec3<f32>(r, g, b) * falloff, 1.0);
}
