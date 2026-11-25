// Compositing a group target back into the target beneath it.
//
// A textured quad with an optional filter matrix or tint, the group's opacity, and the clip every
// other pipeline applies. The sampler filters, because a group target may be held at half
// resolution.

struct CompositeParams {
    // The quad, in device pixels: origin then extent.
    bounds: vec4<f32>,
    // xy: the sampled target's extent in texels. z: how many of those one device pixel covers.
    // w: the multiplier on the whole composite's alpha.
    source: vec4<f32>,
    // x: the clip slot's bits. y: the flags. zw: how far the sampled content is displaced, in
    // device pixels.
    control: vec4<f32>,
    // The shadow's colour, premultiplied.
    tint: vec4<f32>,
    // The filter matrix, column by column.
    matrix0: vec4<f32>,
    matrix1: vec4<f32>,
    matrix2: vec4<f32>,
    matrix3: vec4<f32>,
    // Its constant term.
    matrix_offset: vec4<f32>,
    // xy: the device pixel the sampled target's first texel holds. zw: unused.
    origin: vec4<f32>,
    // The texels of the sampled target that were written, as two corners. Reads are clamped to
    // them.
    written: vec4<f32>,
}

@group(1) @binding(0) var<uniform> composite: CompositeParams;
@group(1) @binding(1) var isolated: texture_2d<f32>;
@group(1) @binding(2) var isolated_sampler: sampler;

// Replace what was sampled with a flat colour scaled by its alpha, which is a drop shadow.
const COMPOSITE_TINT: u32 = 1u;
// Run the sampled colour through the filter matrix.
const COMPOSITE_MATRIX: u32 = 2u;
// Read transparent outside the written texels.
const COMPOSITE_TRANSPARENT: u32 = 4u;

struct CompositeVarying {
    @builtin(position) position: vec4<f32>,
    // `device` is a reserved word in Metal, and member names reach the MSL output verbatim.
    @location(0) device_point: vec2<f32>,
}

@vertex
fn vs_composite(@builtin(vertex_index) vertex: u32) -> CompositeVarying {
    let corner = cr_unit_corner(vertex);
    let device = composite.bounds.xy + corner * composite.bounds.zw;
    var out: CompositeVarying;
    out.position = cr_device_to_target(device);
    out.device_point = device;
    return out;
}

@fragment
fn fs_composite(in: CompositeVarying) -> @location(0) vec4<f32> {
    let coverage = cr_clip_coverage(in.device_point, bitcast<u32>(composite.control.x));
    if coverage <= 0.0 {
        return vec4<f32>(0.0);
    }
    let flags = u32(composite.control.y);
    let sampled = sample_isolated(in.device_point - composite.control.zw);

    var color = sampled;
    if (flags & COMPOSITE_TINT) != 0u {
        color = composite.tint * sampled.a;
    } else if (flags & COMPOSITE_MATRIX) != 0u {
        color = apply_filter_matrix(sampled);
    }
    return color * coverage * composite.source.w;
}

// The isolated target at a device position, through the same grid it was written on.
fn sample_isolated(device: vec2<f32>) -> vec4<f32> {
    let texel = (device - composite.origin.xy) * composite.source.z;
    let low = composite.written.xy + vec2<f32>(0.5);
    let high = max(composite.written.zw - vec2<f32>(0.5), low);
    let uv = clamp(texel, low, high) / composite.source.xy;
    let sampled = textureSampleLevel(isolated, isolated_sampler, uv, 0.0);
    if (u32(composite.control.y) & COMPOSITE_TRANSPARENT) == 0u {
        return sampled;
    }
    // Fades out over the device pixel past the written texels.
    let scale = composite.source.z;
    let inside = clamp((texel - composite.written.xy) / scale + vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(1.0))
        * clamp((composite.written.zw - texel) / scale + vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(1.0));
    return sampled * inside.x * inside.y;
}

// The filter matrix, applied to straight colour and premultiplied again.
fn apply_filter_matrix(premultiplied: vec4<f32>) -> vec4<f32> {
    let alpha = premultiplied.a;
    if alpha <= 0.0 {
        return vec4<f32>(0.0);
    }
    let straight = vec4<f32>(premultiplied.rgb / alpha, alpha);
    let filtered = clamp(
        composite.matrix0 * straight.r
            + composite.matrix1 * straight.g
            + composite.matrix2 * straight.b
            + composite.matrix3 * straight.a
            + composite.matrix_offset,
        vec4<f32>(0.0),
        vec4<f32>(1.0),
    );
    return vec4<f32>(filtered.rgb * filtered.a, filtered.a);
}
