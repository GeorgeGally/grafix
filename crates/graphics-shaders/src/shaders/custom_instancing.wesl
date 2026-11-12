// Custom GPU instancing: one draw call, per-instance data in a vertex buffer,
// animation evaluated on the GPU from the global time.
import bevy_pbr::render::{
    mesh_functions::{get_world_from_local, mesh_position_local_to_clip},
    mesh_view_bindings::globals,
};

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,

    @location(3) i_pos_scale: vec4<f32>,
    @location(4) i_color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
};

fn rotate_y(p: vec3<f32>, a: f32) -> vec3<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec3<f32>(c * p.x + s * p.z, p.y, -s * p.x + c * p.z);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    // Per-instance phase lives in the colour's alpha channel.
    let phase = vertex.i_color.a * 6.2831853;
    let t = globals.time;
    let spin = t * 0.8 + phase;
    let local = rotate_y(vertex.position * vertex.i_pos_scale.w, spin);
    let bob = sin(t * 1.3 + phase) * 0.25;
    let position = local + vertex.i_pos_scale.xyz + vec3<f32>(0.0, bob, 0.0);

    var out: VertexOutput;
    // Instance 0 of the mesh uniform array holds the clone root's transform.
    out.clip_position = mesh_position_local_to_clip(get_world_from_local(0u), vec4<f32>(position, 1.0));
    out.color = vec4<f32>(vertex.i_color.rgb, 1.0);
    out.normal = rotate_y(vertex.normal, spin);
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let key = max(dot(n, normalize(vec3<f32>(0.4, 0.8, 0.3))), 0.0);
    let fill = max(dot(n, normalize(vec3<f32>(-0.6, 0.2, -0.5))), 0.0) * 0.25;
    return vec4<f32>(in.color.rgb * (0.12 + key + fill), 1.0);
}
