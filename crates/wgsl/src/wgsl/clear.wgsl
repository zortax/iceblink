// Clearing one damage rectangle by drawing transparent over the scissor with no blend, since a
// render pass has no scissored clear. It has no bindings.

struct ClearVarying {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs_clear(@builtin(vertex_index) vertex: u32) -> ClearVarying {
    let corner = vec2<f32>(f32(vertex & 1u), 0.5 * f32(vertex & 2u));
    var out: ClearVarying;
    out.position = vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_clear(in: ClearVarying) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}
