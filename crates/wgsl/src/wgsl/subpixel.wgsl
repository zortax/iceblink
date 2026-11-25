// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders_subpixel.wgsl (Apache-2.0)
// The dual-source blend formulation, colour on one output and per-channel coverage on the other so
// the blend factor is the coverage itself, and the display-order swap are adapted from that work,
// which is licensed under the Apache License, Version 2.0, and have been modified: the coverage
// correction is shared with the single-channel pipeline, the sprite is filled with a paint, and the
// clip is evaluated by the shared coverage function.

@group(1) @binding(0) var sprites: texture_2d<u32>;

struct SubpixelOutput {
    @location(0) @blend_src(0) color: vec4<f32>,
    @location(0) @blend_src(1) coverage: vec4<f32>,
}

@vertex
fn vs_subpixel_sprite(
    @builtin(vertex_index) vertex: u32,
    @location(0) slot: u32,
    @location(1) shift: vec2<f32>,
) -> SpriteVarying {
    return sprite_corner(vertex, slot, shift);
}

@fragment
fn fs_subpixel_sprite(in: SpriteVarying) -> SubpixelOutput {
    let clip = cr_clip_coverage(cr_device_position(in.position.xy), in.v_clip);
    let color = sprite_paint(in);
    let straight = straight_rgb(color);

    var sample = sample_atlas(in.texel, in.v_tile, 0.0).rgb;
    if cr_globals.text.z != 0.0 {
        sample = sample.bgr;
    }
    let coverage = correct_coverage3(sample, straight, cr_globals.text.y) * clip * color.a;

    // The colour is written straight and the coverage is the blend factor, so the result is
    // defined only against an opaque destination.
    var out: SubpixelOutput;
    out.color = vec4<f32>(straight, 1.0);
    out.coverage = vec4<f32>(coverage, 1.0);
    return out;
}
