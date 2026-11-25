// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders.wgsl (Apache-2.0)
// The sprite instance layout, the unit-square expansion and the tile lookup are adapted from that
// work, which is licensed under the Apache License, Version 2.0, and have been modified: a tile is
// addressed in texels of its atlas page rather than in a normalised rectangle, sprites are filled
// with a paint, and the clip is evaluated by the shared coverage function.

// A coverage sprite: a glyph mask or a path mask.
struct Sprite {
    bounds: cr_Bounds,
    paint: cr_PaintRef,
    paint_origin: cr_Vector2,
    tile: cr_Tile,
    clip: u32,
    space: u32,
    _pad: u32,
}

// A full-colour sprite: a colour glyph or an image. Its texels are premultiplied.
struct ColorSprite {
    bounds: cr_Bounds,
    frame: cr_Bounds,
    radii: cr_Radii,
    shapes: cr_Shapes,
    tile: cr_Tile,
    paint: cr_PaintRef,
    paint_origin: cr_Vector2,
    opacity: f32,
    clip: u32,
    space: u32,
    flags: u32,
}

@group(2) @binding(0) var atlas: texture_2d<f32>;
@group(2) @binding(1) var atlas_sampler: sampler;

const SPRITE_GRAYSCALE: u32 = 1u;
const SPRITE_MASK: u32 = 2u;
const SPRITE_REPEAT_X: u32 = 4u;
const SPRITE_REPEAT_Y: u32 = 8u;
const SPRITE_NEAREST: u32 = 16u;
const SPRITE_PAINT_ALPHA: u32 = 32u;

// One `Sprite`, which spans 4 texels.
fn load_sprite(slot: u32) -> Sprite {
    let base = slot * 4u;
    let t0 = cr_texel_f32(textureLoad(sprites, cr_table_texel(base + 0u), 0));
    let t1 = textureLoad(sprites, cr_table_texel(base + 1u), 0);
    let t2 = textureLoad(sprites, cr_table_texel(base + 2u), 0);
    let t3 = textureLoad(sprites, cr_table_texel(base + 3u), 0);
    return Sprite(
        cr_Bounds(t0.x, t0.y, t0.z, t0.w),
        cr_PaintRef(cr_paint_kind(t1.x), t1.y),
        cr_Vector2(bitcast<f32>(t1.z), bitcast<f32>(t1.w)),
        cr_Tile(t2.x, t2.y, t2.z, t2.w, t3.x),
        t3.y,
        t3.z,
        t3.w,
    );
}

// A coverage sprite's varyings.
struct SpriteVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) texel: vec2<f32>,
    // The point, from the sprite's origin in its space.
    @location(1) local: vec2<f32>,
    @location(2) @interpolate(flat) v_tile: vec4<f32>,
    // The paint's colour where the paint is solid.
    @location(3) @interpolate(flat) v_color: vec4<f32>,
    @location(4) @interpolate(flat) v_clip: u32,
    @location(5) @interpolate(flat) v_paint: vec2<u32>,
    // The sprite's origin, then the paint origin.
    @location(6) @interpolate(flat) v_origins: vec4<f32>,
}

// The varyings of one corner of a coverage sprite, drawn at its exact extent: the coverage in the
// tile already holds the edge.
fn sprite_corner(vertex: u32, slot: u32, shift: vec2<f32>) -> SpriteVarying {
    let sprite = load_sprite(slot);
    let corner = cr_unit_corner(vertex);
    let local = cr_bounds_origin(sprite.bounds) + corner * cr_bounds_size(sprite.bounds) + shift;
    var out: SpriteVarying;
    out.position = cr_to_clip_position(local, sprite.space);
    out.v_tile = cr_tile_rect(sprite.tile);
    out.texel = tile_texel(corner, out.v_tile);
    out.local = corner * cr_bounds_size(sprite.bounds);
    var color = vec4<f32>(0.0);
    if sprite.paint.kind == cr_PAINT_SOLID {
        color = cr_rgba_of(cr_load_paint(sprite.paint.index).color);
    }
    out.v_color = color;
    out.v_clip = sprite.clip;
    out.v_paint = vec2<u32>(sprite.paint.kind, sprite.paint.index);
    out.v_origins = vec4<f32>(cr_bounds_origin(sprite.bounds), cr_vector2_of(sprite.paint_origin));
    return out;
}

// The paint of a coverage sprite at its fragment.
fn sprite_paint(in: SpriteVarying) -> vec4<f32> {
    if in.v_paint.x == cr_PAINT_SOLID {
        return in.v_color;
    }
    return cr_paint_color(cr_PaintRef(in.v_paint.x, in.v_paint.y), in.v_origins.xy + in.local, in.v_origins.zw);
}

// Where in the atlas a unit-square corner reads from, in texels.
fn tile_texel(corner: vec2<f32>, rect: vec4<f32>) -> vec2<f32> {
    return rect.xy + corner * rect.zw;
}

// The atlas at a texel position and an explicit level of detail.
//
// The position is clamped to the tile's inner half-texel, so a filtering sampler never reads a
// neighbouring tile. The level is explicit because callers have branched on the clip already.
fn sample_atlas(texel: vec2<f32>, rect: vec4<f32>, lod: f32) -> vec4<f32> {
    let inset = min(vec2<f32>(0.5), rect.zw * 0.5);
    let clamped = clamp(texel, rect.xy + inset, rect.xy + rect.zw - inset);
    return textureSampleLevel(
        atlas,
        atlas_sampler,
        clamped / vec2<f32>(textureDimensions(atlas)),
        lod,
    );
}

// How far from one a texel step may be before the tile is resampled.
const RESAMPLE_SLACK: f32 = 0.01;

// The atlas at a texel position, for a sprite whose texels advance by `step` per pixel.
//
// At one texel per pixel the texel is read as it is. Otherwise the four texels around the
// position are weighted by distance, each read at its centre and clamped to the tile.
fn sample_atlas_stepped(texel: vec2<f32>, rect: vec4<f32>, step: vec2<f32>) -> vec4<f32> {
    if all(abs(step - vec2<f32>(1.0)) <= vec2<f32>(RESAMPLE_SLACK)) {
        return sample_atlas(texel, rect, 0.0);
    }
    let at = texel - vec2<f32>(0.5);
    let base = floor(at) + vec2<f32>(0.5);
    let weight = at - floor(at);
    let low = mix(
        sample_atlas(base, rect, 0.0),
        sample_atlas(base + vec2<f32>(1.0, 0.0), rect, 0.0),
        weight.x,
    );
    let high = mix(
        sample_atlas(base + vec2<f32>(0.0, 1.0), rect, 0.0),
        sample_atlas(base + vec2<f32>(1.0, 1.0), rect, 0.0),
        weight.x,
    );
    return mix(low, high, weight.y);
}

// The straight colour a premultiplied colour stands for.
fn straight_rgb(color: vec4<f32>) -> vec3<f32> {
    if color.a <= 0.0 {
        return color.rgb;
    }
    return color.rgb / color.a;
}
