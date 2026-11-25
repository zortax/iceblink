// The copy from the kept target into a presented image.
//
// A `textureLoad` at the fragment's own coordinate with no blend, so the copy changes no value. It
// is drawn once per rectangle of the image's debt.

@group(0) @binding(0) var composed: texture_2d<f32>;

struct BlitVarying {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs_blit(@builtin(vertex_index) vertex: u32) -> BlitVarying {
    let corner = vec2<f32>(f32(vertex & 1u), 0.5 * f32(vertex & 2u));
    var out: BlitVarying;
    out.position = vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_blit(in: BlitVarying) -> @location(0) vec4<f32> {
    return textureLoad(composed, vec2<i32>(in.position.xy), 0);
}

// The same copy with the attachment's encode cancelled in advance, for a surface that offers no
// unencoded format and cannot be viewed as one.
@fragment
fn fs_blit_undo_srgb(in: BlitVarying) -> @location(0) vec4<f32> {
    let texel = textureLoad(composed, vec2<i32>(in.position.xy), 0);
    return vec4<f32>(srgb_to_linear(texel.rgb), texel.a);
}

fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        srgb_channel_to_linear(color.r),
        srgb_channel_to_linear(color.g),
        srgb_channel_to_linear(color.b),
    );
}

fn srgb_channel_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        return value / 12.92;
    }
    return pow((value + 0.055) / 1.055, 2.4);
}
