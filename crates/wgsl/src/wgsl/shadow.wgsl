// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders.wgsl (Apache-2.0)
// The analytic blurred rounded rectangle — the `erf` approximation, the closed-form horizontal
// integral, the four-sample vertical accumulation and the inset complement — is adapted from that
// work, which is licensed under the Apache License, Version 2.0, and has been modified: the
// scanline's horizontal extent is the ellipse equation rather than the circle's, so a corner with
// two different semi-axes blurs correctly, and it reduces to that work's exact expression when the
// two are equal.

struct Shadow {
    bounds: cr_Bounds,
    shape_bounds: cr_Bounds,
    radii: cr_Radii,
    caster_bounds: cr_Bounds,
    caster_radii: cr_Radii,
    shapes: cr_Shapes,
    color: cr_Rgba,
    blur: f32,
    clip: u32,
    space: u32,
    inset: u32,
}

@group(1) @binding(0) var shadows: texture_2d<u32>;

// One `FrameShadow`, which is 10 texels.
fn load_shadow(slot: u32) -> Shadow {
    let base = slot * 10u;
    let t0 = cr_texel_f32(textureLoad(shadows, cr_table_texel(base + 0u), 0));
    let t1 = cr_texel_f32(textureLoad(shadows, cr_table_texel(base + 1u), 0));
    let t2 = cr_texel_f32(textureLoad(shadows, cr_table_texel(base + 2u), 0));
    let t3 = cr_texel_f32(textureLoad(shadows, cr_table_texel(base + 3u), 0));
    let t4 = cr_texel_f32(textureLoad(shadows, cr_table_texel(base + 4u), 0));
    let t5 = cr_texel_f32(textureLoad(shadows, cr_table_texel(base + 5u), 0));
    let t6 = cr_texel_f32(textureLoad(shadows, cr_table_texel(base + 6u), 0));
    let t7 = cr_texel_f32(textureLoad(shadows, cr_table_texel(base + 7u), 0));
    let t8 = cr_texel_f32(textureLoad(shadows, cr_table_texel(base + 8u), 0));
    let t9 = textureLoad(shadows, cr_table_texel(base + 9u), 0);
    return Shadow(
        cr_Bounds(t0.x, t0.y, t0.z, t0.w),
        cr_Bounds(t1.x, t1.y, t1.z, t1.w),
        cr_Radii(t2.x, t2.y, t2.z, t2.w, t3.x, t3.y, t3.z, t3.w),
        cr_Bounds(t4.x, t4.y, t4.z, t4.w),
        cr_Radii(t5.x, t5.y, t5.z, t5.w, t6.x, t6.y, t6.z, t6.w),
        cr_Shapes(t7.x, t7.y, t7.z, t7.w),
        cr_Rgba(t8.x, t8.y, t8.z, t8.w),
        bitcast<f32>(t9.x),
        t9.y,
        t9.z,
        t9.w,
    );
}

struct ShadowVarying {
    @builtin(position) position: vec4<f32>,
    // The point, from the shadow's origin in the space it was encoded in.
    @location(0) local: vec2<f32>,
    // The shadow's origin in that space.
    @location(1) @interpolate(flat) origin: vec2<f32>,
    // The instance's record, so the fragment stage does not load it again.
    @location(2) @interpolate(flat) v_shape_bounds: vec4<f32>,
    @location(3) @interpolate(flat) v_radii_lo: vec4<f32>,
    @location(4) @interpolate(flat) v_radii_hi: vec4<f32>,
    @location(5) @interpolate(flat) v_caster_bounds: vec4<f32>,
    @location(6) @interpolate(flat) v_caster_radii_lo: vec4<f32>,
    @location(7) @interpolate(flat) v_caster_radii_hi: vec4<f32>,
    @location(8) @interpolate(flat) v_color: vec4<f32>,
    @location(9) @interpolate(flat) v_blur: f32,
    @location(10) @interpolate(flat) v_clip_inset: vec2<u32>,
    @location(11) @interpolate(flat) v_shapes: vec4<f32>,
}

@vertex
fn vs_shadow(
    @builtin(vertex_index) vertex: u32,
    @location(0) slot: u32,
    @location(1) shift: vec2<f32>,
) -> ShadowVarying {
    let shadow = load_shadow(slot);
    // `bounds` is the shadow's ink.
    let local = cr_inflated_corner(vertex, shadow.bounds) + shift;
    var out: ShadowVarying;
    out.position = cr_to_clip_position(local, shadow.space);
    out.local = cr_inflated_offset(vertex, shadow.bounds);
    out.origin = cr_bounds_origin(shadow.bounds);
    out.v_shape_bounds = cr_bounds_of(shadow.shape_bounds);
    out.v_radii_lo = cr_radii_lo(shadow.radii);
    out.v_radii_hi = cr_radii_hi(shadow.radii);
    out.v_caster_bounds = cr_bounds_of(shadow.caster_bounds);
    out.v_caster_radii_lo = cr_radii_lo(shadow.caster_radii);
    out.v_caster_radii_hi = cr_radii_hi(shadow.caster_radii);
    out.v_color = cr_rgba_of(shadow.color);
    out.v_blur = shadow.blur;
    out.v_shapes = cr_shapes_of(shadow.shapes);
    out.v_clip_inset = vec2<u32>(shadow.clip, shadow.inset);
    return out;
}

// A standard gaussian, used to weight the vertical samples.
fn gaussian(x: f32, sigma: f32) -> f32 {
    return exp(-(x * x) / (2.0 * sigma * sigma)) / (sqrt(2.0 * cr_M_PI) * sigma);
}

// An approximation of the error function, which is the integral the gaussian needs.
fn erf(v: vec2<f32>) -> vec2<f32> {
    let s = sign(v);
    let a = abs(v);
    let r1 = 1.0 + (0.278393 + (0.230389 + (0.000972 + 0.078108 * a) * a) * a) * a;
    let r2 = r1 * r1;
    return s - s / (r2 * r2);
}

// The blurred coverage of one scanline of a rounded rectangle, exactly analytic in x.
//
// `curved` is where the shape's edge sits on this scanline: the superellipse's own edge, which is
// the ellipse's at exponent two and the circle's when the two semi-axes are equal.
fn blur_along_x(
    x: f32,
    y: f32,
    sigma: f32,
    corner: vec2<f32>,
    half_size: vec2<f32>,
    shape: f32,
) -> f32 {
    let delta = min(half_size.y - corner.y - abs(y), 0.0);
    var curved = half_size.x - corner.x;
    if corner.y > 0.0 {
        if shape == cr_CORNER_ROUND {
            let normalised = saturate(1.0 - (delta * delta) / (corner.y * corner.y));
            curved += corner.x * sqrt(normalised);
        } else {
            // `|x/rx|^n + |y/ry|^n = 1` solved for x, which is the ellipse's own solution at two.
            let power = clamp(shape, 0.01, 64.0);
            let along = saturate(abs(delta) / corner.y);
            let remaining = saturate(1.0 - pow(along, power));
            curved += corner.x * pow(remaining, 1.0 / power);
        }
    } else {
        curved += corner.x;
    }
    let integral = 0.5 + 0.5 * erf((x + vec2<f32>(-curved, curved)) * (sqrt(0.5) / sigma));
    return integral.y - integral.x;
}

@fragment
fn fs_shadow(in: ShadowVarying) -> @location(0) vec4<f32> {
    let clip = cr_clip_coverage(cr_device_position(in.position.xy), in.v_clip_inset.x);
    if clip <= 0.0 {
        return vec4<f32>(0.0);
    }

    // All distances are from the shadow's origin, so they keep their precision far from the
    // origin of the space.
    let casting = cr_bounds_from(cr_bounds_from_vec4(in.v_shape_bounds), in.origin);
    let caster = cr_bounds_from(cr_bounds_from_vec4(in.v_caster_bounds), in.origin);
    let half_size = cr_bounds_size(casting) * 0.5;
    let center = cr_bounds_origin(casting) + half_size;
    let local = in.local;
    let center_to_point = local - center;
    let radii = cr_radii_from_vec4(in.v_radii_lo, in.v_radii_hi);
    let caster_radii = cr_radii_from_vec4(in.v_caster_radii_lo, in.v_caster_radii_hi);
    let corner = cr_pick_corner_radii(center_to_point, radii);
    let shapes = cr_shapes_from_vec4(in.v_shapes);
    let corner_shape = cr_pick_corner_shape(center_to_point, shapes);
    let blur = in.v_blur;

    // A drop shadow paints nothing inside its caster, so the integral is skipped there.
    let caster_distance = cr_quad_sdf(local, caster, caster_radii, shapes);
    if in.v_clip_inset.y == 0u && caster_distance <= -0.5 {
        return vec4<f32>(0.0);
    }

    var alpha: f32;
    if blur <= 0.0 {
        alpha = saturate(0.5 - cr_quad_sdf(local, casting, radii, shapes));
    } else {
        // The samples are spent where three deviations of the gaussian overlap the shape.
        let low = center_to_point.y - half_size.y;
        let high = center_to_point.y + half_size.y;
        let start = clamp(-3.0 * blur, low, high);
        let end = clamp(3.0 * blur, low, high);
        let step = (end - start) / 4.0;
        var y = start + step * 0.5;
        alpha = 0.0;
        for (var i = 0; i < 4; i += 1) {
            let blurred = blur_along_x(
                center_to_point.x,
                center_to_point.y - y,
                blur,
                corner,
                half_size,
                corner_shape,
            );
            alpha += blurred * gaussian(y, blur) * step;
            y += step;
        }
    }

    if in.v_clip_inset.y != 0u {
        // An inset shadow is the complement of the blurred hole, cut to its caster.
        alpha = 1.0 - alpha;
        alpha *= saturate(0.5 - caster_distance);
    } else {
        // A drop shadow is never painted inside its caster.
        alpha *= saturate(0.5 + caster_distance);
    }

    return in.v_color * alpha * clip;
}

