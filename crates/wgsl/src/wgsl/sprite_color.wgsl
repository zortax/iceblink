// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders.wgsl (Apache-2.0)
// The two stages of the polychrome sprite pipeline are adapted from that work, which is licensed
// under the Apache License, Version 2.0, and have been modified: the tile is addressed in atlas
// texels rather than normalised coordinates, the clip is evaluated by a shared coverage function
// rather than four interpolated distances, the texels are premultiplied, the picture can repeat,
// tint a mask or take a paint's alpha, and the rounded frame has elliptical semi-axes and an
// exponent per corner.

// Full-colour sprites: colour glyphs and images.

@group(1) @binding(0) var sprites: texture_2d<u32>;

// One `ColorSprite`, which the arena pads to 9 texels.
fn load_color_sprite(slot: u32) -> ColorSprite {
    let base = slot * 9u;
    let t0 = cr_texel_f32(textureLoad(sprites, cr_table_texel(base + 0u), 0));
    let t1 = cr_texel_f32(textureLoad(sprites, cr_table_texel(base + 1u), 0));
    let t2 = cr_texel_f32(textureLoad(sprites, cr_table_texel(base + 2u), 0));
    let t3 = cr_texel_f32(textureLoad(sprites, cr_table_texel(base + 3u), 0));
    let t4 = cr_texel_f32(textureLoad(sprites, cr_table_texel(base + 4u), 0));
    let t5 = textureLoad(sprites, cr_table_texel(base + 5u), 0);
    let t6 = textureLoad(sprites, cr_table_texel(base + 6u), 0);
    let t7 = textureLoad(sprites, cr_table_texel(base + 7u), 0);
    let t8 = textureLoad(sprites, cr_table_texel(base + 8u), 0);
    return ColorSprite(
        cr_Bounds(t0.x, t0.y, t0.z, t0.w),
        cr_Bounds(t1.x, t1.y, t1.z, t1.w),
        cr_Radii(t2.x, t2.y, t2.z, t2.w, t3.x, t3.y, t3.z, t3.w),
        cr_Shapes(t4.x, t4.y, t4.z, t4.w),
        cr_Tile(t5.x, t5.y, t5.z, t5.w, t6.x),
        cr_PaintRef(cr_paint_kind(t6.y), t6.z),
        cr_Vector2(bitcast<f32>(t6.w), bitcast<f32>(t7.x)),
        bitcast<f32>(t7.y),
        t7.z,
        t7.w,
        t8.x,
    );
}

struct ColorSpriteVarying {
    @builtin(position) position: vec4<f32>,
    // The point, from the sprite's origin in its space.
    @location(0) local: vec2<f32>,
    // The sprite's origin in its space.
    @location(1) @interpolate(flat) origin: vec2<f32>,
    @location(2) @interpolate(flat) v_tile: vec4<f32>,
    @location(3) @interpolate(flat) v_frame: vec4<f32>,
    @location(4) @interpolate(flat) v_radii_lo: vec4<f32>,
    @location(5) @interpolate(flat) v_radii_hi: vec4<f32>,
    @location(6) @interpolate(flat) v_shapes: vec4<f32>,
    // The clip, the flags, then the paint.
    @location(7) @interpolate(flat) v_ids: vec4<u32>,
    // The bounds' extent, the opacity, and an unused lane.
    @location(8) @interpolate(flat) v_extent: vec4<f32>,
    @location(9) @interpolate(flat) v_paint_origin: vec2<f32>,
}

// The span of one axis the sprite paints: the frame on a repeating axis, and otherwise the bounds
// cut to the frame.
fn painted_span(bounds_start: f32, bounds_size: f32, frame_start: f32, frame_size: f32, repeats: bool) -> vec2<f32> {
    if repeats {
        return vec2<f32>(frame_start, frame_start + frame_size);
    }
    let low = max(bounds_start, frame_start);
    let high = max(min(bounds_start + bounds_size, frame_start + frame_size), low);
    return vec2<f32>(low, high);
}

@vertex
fn vs_color_sprite(
    @builtin(vertex_index) vertex: u32,
    @location(0) slot: u32,
    @location(1) shift: vec2<f32>,
) -> ColorSpriteVarying {
    let sprite = load_color_sprite(slot);
    let x = painted_span(
        sprite.bounds.x, sprite.bounds.w, sprite.frame.x, sprite.frame.w,
        (sprite.flags & SPRITE_REPEAT_X) != 0u,
    );
    let y = painted_span(
        sprite.bounds.y, sprite.bounds.h, sprite.frame.y, sprite.frame.h,
        (sprite.flags & SPRITE_REPEAT_Y) != 0u,
    );
    let corner = cr_unit_corner(vertex);
    let painted = vec2<f32>(mix(x.x, x.y, corner.x), mix(y.x, y.y, corner.y));
    let origin = cr_bounds_origin(sprite.bounds);
    var out: ColorSpriteVarying;
    out.position = cr_to_clip_position(painted + shift, sprite.space);
    out.local = painted - origin;
    out.origin = origin;
    out.v_tile = cr_tile_rect(sprite.tile);
    out.v_frame = cr_bounds_of(sprite.frame);
    out.v_radii_lo = cr_radii_lo(sprite.radii);
    out.v_radii_hi = cr_radii_hi(sprite.radii);
    out.v_shapes = cr_shapes_of(sprite.shapes);
    out.v_ids = vec4<u32>(sprite.clip, sprite.flags, sprite.paint.kind, sprite.paint.index);
    out.v_extent = vec4<f32>(cr_bounds_size(sprite.bounds), sprite.opacity, 0.0);
    out.v_paint_origin = cr_vector2_of(sprite.paint_origin);
    return out;
}

@fragment
fn fs_color_sprite(in: ColorSpriteVarying) -> @location(0) vec4<f32> {
    let flags = in.v_ids.y;
    let extent = max(in.v_extent.xy, vec2<f32>(1.0e-6));
    var uv = in.local / extent;
    // Derivatives of the unwrapped position, taken while control flow is uniform.
    let texel_dx = dpdx(uv * in.v_tile.zw);
    let texel_dy = dpdy(uv * in.v_tile.zw);
    let lod = 0.5 * log2(max(max(dot(texel_dx, texel_dx), dot(texel_dy, texel_dy)), 1.0));
    let clip = cr_clip_coverage(cr_device_position(in.position.xy), in.v_ids.x);
    if clip <= 0.0 {
        return vec4<f32>(0.0);
    }
    let repeat = vec2<bool>((flags & SPRITE_REPEAT_X) != 0u, (flags & SPRITE_REPEAT_Y) != 0u);
    uv = select(uv, fract(uv), repeat);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return vec4<f32>(0.0);
    }
    var texel_at = in.v_tile.xy + uv * in.v_tile.zw;
    var level = lod;
    if (flags & SPRITE_NEAREST) != 0u {
        texel_at = floor(min(texel_at, in.v_tile.xy + in.v_tile.zw - vec2<f32>(0.5))) + vec2<f32>(0.5);
        level = 0.0;
    }
    var texel = sample_atlas(texel_at, in.v_tile, level);
    let point = in.origin + in.local;
    let paint = cr_PaintRef(in.v_ids.z, in.v_ids.w);
    var color = texel;
    if (flags & SPRITE_MASK) != 0u {
        color = cr_paint_color(paint, point, in.v_paint_origin) * texel.a;
    } else if (flags & SPRITE_GRAYSCALE) != 0u {
        let gray = color_brightness(straight_rgb(texel));
        color = vec4<f32>(vec3<f32>(gray) * texel.a, texel.a);
    }
    if (flags & SPRITE_PAINT_ALPHA) != 0u {
        color *= cr_paint_color(paint, point, in.v_paint_origin).a;
    }
    let frame = cr_rect_coverage(
        in.local,
        cr_bounds_from(cr_bounds_from_vec4(in.v_frame), in.origin),
        cr_radii_from_vec4(in.v_radii_lo, in.v_radii_hi),
        cr_shapes_from_vec4(in.v_shapes),
    );
    return color * saturate(in.v_extent.z) * frame * clip;
}
