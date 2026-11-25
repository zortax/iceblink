// A rectangle showing a texture the renderer did not draw, such as a video frame.
//
// It binds its own texture and block instead of being instanced, and shares the clip with every
// other pipeline.

struct ExternalParams {
    // The quad in its space: origin then extent.
    bounds: vec4<f32>,
    // x: the clip slot's bits. y: the space slot's bits. z: the multiplier on the texture's alpha.
    // w: non-zero when the texture's colours are already scaled by its alpha.
    control: vec4<f32>,
}

@group(1) @binding(0) var<uniform> foreign: ExternalParams;
@group(1) @binding(1) var external_texture: texture_2d<f32>;
@group(1) @binding(2) var external_sampler: sampler;

struct ExternalVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_external(@builtin(vertex_index) vertex: u32) -> ExternalVarying {
    let corner = cr_unit_corner(vertex);
    let local = foreign.bounds.xy + corner * foreign.bounds.zw;
    var out: ExternalVarying;
    out.position = cr_to_clip_position(local, bitcast<u32>(foreign.control.y));
    out.uv = corner;
    return out;
}

@fragment
fn fs_external(in: ExternalVarying) -> @location(0) vec4<f32> {
    let coverage = cr_clip_coverage(cr_device_position(in.position.xy), bitcast<u32>(foreign.control.x));
    if coverage <= 0.0 {
        return vec4<f32>(0.0);
    }
    var texel = textureSampleLevel(external_texture, external_sampler, in.uv, 0.0);
    if foreign.control.w == 0.0 {
        texel = vec4<f32>(texel.rgb * texel.a, texel.a);
    }
    return texel * coverage * foreign.control.z;
}
