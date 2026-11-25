// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders.wgsl (Apache-2.0)
// The two stages of the monochrome sprite pipeline are adapted from that work, which is licensed
// under the Apache License, Version 2.0, and have been modified: the tile is addressed in atlas
// texels rather than normalised coordinates, the clip is evaluated by a shared coverage function
// rather than four interpolated distances, and the sprite is filled with a paint rather than an
// HSLA quadruple converted in the vertex stage.

// Single-channel coverage sprites.

@group(1) @binding(0) var sprites: texture_2d<u32>;

@vertex
fn vs_mono_sprite(
    @builtin(vertex_index) vertex: u32,
    @location(0) slot: u32,
    @location(1) shift: vec2<f32>,
) -> SpriteVarying {
    return sprite_corner(vertex, slot, shift);
}

@fragment
fn fs_mono_sprite(in: SpriteVarying) -> @location(0) vec4<f32> {
    // Taken before the clip branches: a derivative is defined only in uniform control flow.
    let step = fwidth(in.texel);
    let clip = cr_clip_coverage(cr_device_position(in.position.xy), in.v_clip);
    if clip <= 0.0 {
        return vec4<f32>(0.0);
    }
    let color = sprite_paint(in);
    let sample = sample_atlas_stepped(in.texel, in.v_tile, step).r;
    let coverage = correct_coverage(sample, straight_rgb(color), cr_globals.text.x);
    return color * coverage * clip;
}
