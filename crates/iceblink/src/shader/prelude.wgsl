// The prelude of an application shader: the input it is called with and the helpers it may use.
//
// Every value a shader reads arrives as an argument, and nothing here reads a binding, so the
// prelude works in every mode.

// Where and when a fragment is drawn.
struct ShaderInput {
    // The position in the item's bounds, zero to one on each axis.
    uv: vec2<f32>,
    // The position from the bounds origin, in local units.
    local: vec2<f32>,
    // The bounds' size, at least 1e-4 on each axis.
    size: vec2<f32>,
    // The output's scale factor.
    scale: f32,
    // Seconds from the compositor's start to the frame's present time.
    time: f32,
    // Seconds from the previous frame's present time.
    delta: f32,
    // The pointer, in the same coordinates as `local`.
    pointer: vec2<f32>,
    // One while the pointer is over the item, zero otherwise.
    hovered: f32,
}

// Where the content a filter reads is, and how much of it may be read.
struct FilterSource {
    // The device position of the group's origin.
    origin: vec2<f32>,
    // The device position the source's first texel holds.
    source_origin: vec2<f32>,
    // Texels of the source per device pixel.
    scale: f32,
    // The source's extent, in texels.
    extent: vec2<f32>,
    // The lowest and highest texel that may be read.
    low: vec2<f32>,
    high: vec2<f32>,
}

// The content at `point` from the group's origin, in device pixels, with reads clamped to
// `[low, high]`.
fn source_at(
    source: texture_2d<f32>,
    source_sampler: sampler,
    region: FilterSource,
    point: vec2<f32>,
) -> vec4<f32> {
    let texel = (region.origin + point - region.source_origin) * region.scale;
    let low = region.low + vec2<f32>(0.5);
    let clamped = clamp(texel, low, max(region.high - vec2<f32>(0.5), low));
    return textureSampleLevel(source, source_sampler, clamped / region.extent, 0.0);
}

// A straight colour premultiplied by its alpha.
fn premultiplied(color: vec3<f32>, alpha: f32) -> vec4<f32> {
    return vec4<f32>(color * alpha, alpha);
}

// Coverage of one device pixel by a shape whose signed distance is `distance`, negative inside.
fn coverage_of(distance: f32) -> f32 {
    return saturate(0.5 - distance);
}

// Signed distance from `point` to a superellipse with the given semi-axes and exponent, negative
// inside. Exponent two is an ellipse.
fn superellipse_sdf(point: vec2<f32>, radii: vec2<f32>, exponent: f32) -> f32 {
    let r = max(radii, vec2<f32>(1.0e-4));
    let n = max(exponent, 1.0e-4);
    let normalised = pow(abs(point) / r, vec2<f32>(n));
    let value = pow(max(normalised.x + normalised.y, 1.0e-12), 1.0 / n);
    let gradient = max(length(normalised / max(abs(point), vec2<f32>(1.0e-4))), 1.0e-4);
    return (value - 1.0) / gradient;
}

// An sRGB colour from a hue in turns, a saturation and a lightness, each from zero to one.
fn hsl(hue: f32, saturation: f32, lightness: f32) -> vec3<f32> {
    let h = fract(hue) * 6.0;
    let c = (1.0 - abs(2.0 * lightness - 1.0)) * saturation;
    // A modulus with the sign of its left operand.
    let wrapped = h - 2.0 * trunc(h / 2.0);
    let x = c * (1.0 - abs(wrapped - 1.0));
    var rgb = vec3<f32>(0.0);
    if h < 1.0 {
        rgb = vec3<f32>(c, x, 0.0);
    } else if h < 2.0 {
        rgb = vec3<f32>(x, c, 0.0);
    } else if h < 3.0 {
        rgb = vec3<f32>(0.0, c, x);
    } else if h < 4.0 {
        rgb = vec3<f32>(0.0, x, c);
    } else if h < 5.0 {
        rgb = vec3<f32>(x, 0.0, c);
    } else {
        rgb = vec3<f32>(c, 0.0, x);
    }
    return rgb + vec3<f32>(lightness - 0.5 * c);
}
