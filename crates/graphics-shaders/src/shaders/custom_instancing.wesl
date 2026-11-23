// Custom GPU instancing: one draw call, per-instance data in a vertex buffer,
// animation evaluated on the GPU from the global time.
import bevy_pbr::render::{
    mesh_view_bindings::globals,
    view_transformations::position_world_to_clip,
};

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,

    @location(3) i_pos_scale: vec4<f32>,
    @location(4) i_color: vec4<f32>,
    @location(5) i_orientation: vec4<f32>,
    @location(6) i_shape_scale: vec4<f32>,
    @location(7) i_surface: vec4<f32>,
    @location(8) i_emissive: vec4<f32>,
    @location(9) i_playback: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) surface: vec2<f32>,
    @location(3) emissive: vec3<f32>,
};

fn rotate_y(p: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec3<f32>(c * p.x + s * p.z, p.y, -s * p.x + c * p.z);
}

fn rotate_quat(v: vec3<f32>, q: vec4<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    // Per-instance phase lives in the colour's alpha channel.
    let phase = vertex.i_color.a * 6.2831853;
    let t = globals.time * vertex.i_playback.x + vertex.i_playback.y;
    let spin = (t * 0.8 + phase) * vertex.i_shape_scale.w;
    let scaled = vertex.position * vertex.i_shape_scale.xyz * vertex.i_pos_scale.w;
    let local = rotate_quat(rotate_y(scaled, spin), vertex.i_orientation);
    let bob = sin(t * 1.3 + phase) * 0.25 * vertex.i_shape_scale.w;
    let position = local + vertex.i_pos_scale.xyz
        + rotate_quat(vec3<f32>(0.0, bob, 0.0), vertex.i_orientation);

    var out: VertexOutput;
    out.clip_position = position_world_to_clip(position);
    out.color = vec4<f32>(vertex.i_color.rgb, 1.0);
    out.surface = vertex.i_surface.xy;
    out.emissive = vertex.i_emissive.rgb;
    // Inverse-transpose of the nonuniform shape scale keeps PBR normals sane.
    let correct_normal = vertex.normal / max(vertex.i_shape_scale.xyz, vec3<f32>(0.001));
    out.normal = normalize(rotate_quat(rotate_y(correct_normal, spin), vertex.i_orientation));
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    // Fast instanced shader: surface controls affect output, but this is
    // analytic shading rather than Bevy's full physically based pipeline.
    let key_dir = normalize(vec3<f32>(0.4, 0.8, 0.3));
    let key = max(dot(n, key_dir), 0.0);
    let fill = max(dot(n, normalize(vec3<f32>(-0.6, 0.2, -0.5))), 0.0) * 0.25;
    let metallic = clamp(in.surface.x, 0.0, 1.0);
    let roughness = clamp(in.surface.y, 0.02, 1.0);
    let half_way = normalize(key_dir + vec3<f32>(0.0, 0.0, 1.0));
    let shine = pow(max(dot(n, half_way), 0.0), mix(128.0, 3.0, roughness));
    let f0 = mix(vec3<f32>(0.04), in.color.rgb, metallic);
    let diffuse = in.color.rgb * (1.0 - metallic) * (0.12 + key + fill);
    let specular = f0 * shine * (1.0 - roughness * 0.65) * 2.0;
    return vec4<f32>(diffuse + specular + in.emissive, 1.0);
}
