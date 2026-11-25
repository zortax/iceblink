// Converts one accumulated layer from premultiplied to straight colour.
//
// The draw replaces every texel, so the result never depends on what the layer held.

@group(0) @binding(0) var accumulated: texture_2d<f32>;

@vertex
fn vs_resolve(@builtin(vertex_index) vertex: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32(vertex & 1u), 0.5 * f32(vertex & 2u));
    return vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
}

@fragment
fn fs_resolve(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let premultiplied = textureLoad(accumulated, vec2<i32>(position.xy), 0);
    if premultiplied.a <= 0.0 {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(premultiplied.rgb / premultiplied.a, premultiplied.a);
}
