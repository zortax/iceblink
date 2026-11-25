// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders.wgsl (Apache-2.0)
// The Oklab conversion pair and the linear/encoded sRGB transfer functions are adapted from that
// work, which is licensed under the Apache License, Version 2.0, and have been modified: a ramp
// here carries any number of stops rather than exactly two, its stops arrive already expressed in
// the space the ramp is interpolated in, and the result is premultiplied gamma-encoded sRGB rather
// than that work's optionally-premultiplied output.

fn cr_srgb_channel_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        return value / 12.92;
    }
    return pow((value + 0.055) / 1.055, 2.4);
}

fn cr_linear_channel_to_srgb(value: f32) -> f32 {
    if value <= 0.0031308 {
        return value * 12.92;
    }
    return 1.055 * pow(value, 1.0 / 2.4) - 0.055;
}

fn cr_srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        cr_srgb_channel_to_linear(color.r),
        cr_srgb_channel_to_linear(color.g),
        cr_srgb_channel_to_linear(color.b),
    );
}

fn cr_linear_to_srgb(color: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        cr_linear_channel_to_srgb(color.r),
        cr_linear_channel_to_srgb(color.g),
        cr_linear_channel_to_srgb(color.b),
    );
}

// Reference: https://bottosson.github.io/posts/oklab/
fn cr_oklab_to_linear_srgb(color: vec3<f32>) -> vec3<f32> {
    let l_ = color.x + 0.3963377774 * color.y + 0.2158037573 * color.z;
    let m_ = color.x - 0.1055613458 * color.y - 0.0638541728 * color.z;
    let s_ = color.x - 0.0894841775 * color.y - 1.2914855480 * color.z;

    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    return vec3<f32>(
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    );
}

// Where along a ramp a point lies, before the ramp's extent is applied.
fn cr_gradient_position(paint: cr_Paint, point: vec2<f32>) -> f32 {
    if paint.gradient == 0u {
        let start = vec2<f32>(paint.geometry.x, paint.geometry.y);
        let end = vec2<f32>(paint.geometry.z, paint.geometry.w);
        let axis = end - start;
        let length_squared = dot(axis, axis);
        if length_squared <= 0.0 {
            return 0.0;
        }
        return dot(point - start, axis) / length_squared;
    }
    if paint.gradient == 1u {
        let center = vec2<f32>(paint.geometry.x, paint.geometry.y);
        let radii = vec2<f32>(max(paint.geometry.z, 1e-6), max(paint.geometry.w, 1e-6));
        return length((point - center) / radii);
    }
    // Conic: the sweep starts at twelve o'clock and runs clockwise in y-down space.
    let center = vec2<f32>(paint.geometry.x, paint.geometry.y);
    let delta = point - center;
    let angle = atan2(delta.x, -delta.y) - paint.geometry.w;
    return cr_sdf_fmod(angle / (2.0 * cr_M_PI) + 1.0, 1.0);
}

// The stop position a ramp is sampled at, once repetition or clamping has been applied.
fn cr_gradient_extent(paint: cr_Paint, raw: f32) -> f32 {
    if paint.flags == 1u {
        return fract(raw);
    }
    return saturate(raw);
}

// The colour of border side `side` (top, right, bottom, left) of a sides paint, as premultiplied
// gamma-encoded sRGB.
fn cr_side_color(paint: cr_Paint, side: u32) -> vec4<f32> {
    return cr_vector4_of(cr_load_stop(paint.stop_start + side).color);
}

// The colour of a ramp at `t`, as premultiplied gamma-encoded sRGB.
fn cr_sample_ramp(paint: cr_Paint, t: f32) -> vec4<f32> {
    let count = paint.stop_count;
    if count == 0u {
        return vec4<f32>(0.0);
    }
    var low = cr_load_stop(paint.stop_start);
    if count == 1u || t <= low.offset {
        return cr_decode_stop(paint.space, cr_vector4_of(low.color));
    }
    let last = cr_load_stop(paint.stop_start + count - 1u);
    if t >= last.offset {
        return cr_decode_stop(paint.space, cr_vector4_of(last.color));
    }
    var index = 1u;
    loop {
        if index >= count {
            break;
        }
        let high = cr_load_stop(paint.stop_start + index);
        if t <= high.offset {
            let span = max(high.offset - low.offset, 1e-6);
            let mixed = mix(
                cr_vector4_of(low.color),
                cr_vector4_of(high.color),
                saturate((t - low.offset) / span),
            );
            return cr_decode_stop(paint.space, mixed);
        }
        low = high;
        index += 1u;
    }
    return cr_decode_stop(paint.space, cr_vector4_of(low.color));
}

// An interpolated stop, back in premultiplied gamma-encoded sRGB.
//
// Stops are premultiplied in their own encoding, so a ramp to transparent stays its colour.
fn cr_decode_stop(space: u32, value: vec4<f32>) -> vec4<f32> {
    let alpha = value.a;
    if space == cr_SPACE_SRGB {
        return value;
    }
    if alpha <= 0.0 {
        return vec4<f32>(0.0);
    }
    let straight = value.rgb / alpha;
    var linear = straight;
    if space == cr_SPACE_OKLAB {
        linear = cr_oklab_to_linear_srgb(straight);
    }
    return vec4<f32>(cr_linear_to_srgb(max(linear, vec3<f32>(0.0))) * alpha, alpha);
}

// What a paint reference paints at `point`, with `origin` subtracted first.
//
// The kind is stored beside the index, so a solid paint reads no stop.
fn cr_paint_color(reference: cr_PaintRef, point: vec2<f32>, origin: vec2<f32>) -> vec4<f32> {
    if reference.kind == cr_PAINT_NONE {
        return vec4<f32>(0.0);
    }
    let paint = cr_load_paint(reference.index);
    if reference.kind == cr_PAINT_SOLID {
        return cr_rgba_of(paint.color);
    }
    let anchored = point - origin;
    if reference.kind == cr_PAINT_GRADIENT {
        return cr_sample_ramp(paint, cr_gradient_extent(paint, cr_gradient_position(paint, anchored)));
    }
    if reference.kind == cr_PAINT_SIDES {
        // Read anywhere but a border, a sides paint is its top colour.
        return cr_side_color(paint, 0u);
    }
    return vec4<f32>(0.0);
}
