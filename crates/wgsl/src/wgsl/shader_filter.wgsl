// The renderer's half of a filter shader: the block, the stage over the region, and the call.
//
// The content is already in a group target. This pass reads it and writes what replaces it, cut
// to the region by the scissor.

struct cr_FilterParams {
    // xy: the source's extent in texels. zw: the destination's.
    extents: vec4<f32>,
    // xy: the device position of the group's origin. z: texels of the source per device pixel.
    // w: texels of the destination per device pixel.
    placement: vec4<f32>,
    // The part of the source this frame wrote, in source texels: the two corners.
    valid: vec4<f32>,
    // xy: the group's extent in device pixels. z: the shader clock's time. w: its delta.
    group: vec4<f32>,
    // x: the output's scale factor. A filter binds none of the frame tables, so this replaces
    // the globals.
    frame: vec4<f32>,
    // xy: the device pixel the source's first texel holds. zw: the same for the destination.
    origins: vec4<f32>,
}

@group(0) @binding(0) var<uniform> cr_filtered: cr_FilterParams;
@group(0) @binding(1) var cr_filter_source: texture_2d<f32>;
@group(0) @binding(2) var cr_filter_sampler: sampler;

struct cr_ShaderParams {
    // The pointer, from the group's origin in device pixels.
    pointer: vec2<f32>,
    // One while the pointer is over the item.
    hovered: f32,
    // Puts the shader's half on a sixteen-byte boundary.
    reserved: f32,
    // Sized to the whole block whatever the shader declares, as WebGL 2 needs a multiple of sixteen
    // bytes.
    @size(64) user: Params,
}

@group(1) @binding(0) var<uniform> cr_shader_params: cr_ShaderParams;

struct cr_FilterVarying {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn cr_vs_shader_filter(@builtin(vertex_index) vertex: u32) -> cr_FilterVarying {
    let corner = vec2<f32>(f32(vertex & 1u), 0.5 * f32(vertex & 2u));
    var out: cr_FilterVarying;
    out.position = vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
    return out;
}

@fragment
fn cr_fs_shader_filter(in: cr_FilterVarying) -> @location(0) vec4<f32> {
    // The device pixel of this destination texel's centre, then the same point from the group's
    // origin.
    let device = (floor(in.position.xy) + vec2<f32>(0.5)) / cr_filtered.placement.w
        + cr_filtered.origins.zw;
    let local = device - cr_filtered.placement.xy;

    var input: ShaderInput;
    input.local = local;
    input.size = max(cr_filtered.group.xy, vec2<f32>(1.0e-4));
    input.uv = input.local / input.size;
    input.scale = cr_filtered.frame.x;
    input.time = cr_filtered.group.z;
    input.delta = cr_filtered.group.w;
    input.pointer = cr_shader_params.pointer;
    input.hovered = cr_shader_params.hovered;

    var region: FilterSource;
    region.origin = cr_filtered.placement.xy;
    region.source_origin = cr_filtered.origins.xy;
    region.scale = cr_filtered.placement.z;
    region.extent = cr_filtered.extents.xy;
    region.low = cr_filtered.valid.xy;
    region.high = cr_filtered.valid.zw;

    let color = apply(input, cr_shader_params.user, cr_filter_source, cr_filter_sampler, region);
    return clamp(color, vec4<f32>(0.0), vec4<f32>(1.0));
}
