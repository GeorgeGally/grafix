// Minimal compute shader: writes an animated domain-warped field into a
// storage texture that the scene samples as an emissive material.

@group(0) @binding(0) var output: texture_storage_2d<rgba8unorm, write>;

struct FieldParams {
    time: f32,
    speed: f32,
    scale: f32,
    _pad: f32,
}
@group(0) @binding(1) var<uniform> params: FieldParams;

fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash2(i), hash2(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash2(i + vec2<f32>(0.0, 1.0)), hash2(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

fn fbm(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var v = 0.0;
    var a = 0.5;
    for (var i = 0; i < 5; i = i + 1) {
        v = v + a * noise(p);
        p = p * 2.03 + vec2<f32>(1.7, 9.2);
        a = a * 0.5;
    }
    return v;
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(output);
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let uv = vec2<f32>(id.xy) / vec2<f32>(size);
    let t = params.time * params.speed;
    let p = uv * params.scale;
    let warp = vec2<f32>(fbm(p + vec2<f32>(t * 0.3, 0.0)), fbm(p + vec2<f32>(5.2, t * 0.25)));
    let v = fbm(p + 3.0 * warp);
    let col = mix(vec3<f32>(0.02, 0.05, 0.12), vec3<f32>(0.1, 0.65, 0.9), smoothstep(0.3, 0.8, v));
    let hot = mix(col, vec3<f32>(1.0, 0.55, 0.2), smoothstep(0.62, 0.9, v * warp.x * 1.6));
    textureStore(output, vec2<i32>(id.xy), vec4<f32>(hot, 1.0));
}
