// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders.wgsl (Apache-2.0)
// The separable gaussian chain — a snapped 2:1 box downsample followed by two single-axis passes
// whose taps spread out rather than truncating when the radius outruns the tap budget — is adapted
// from that work, which is licensed under the Apache License, Version 2.0, and has been modified:
// every pass takes the source and destination extents and the device-pixel scale of each as
// uniforms, and the sampling lattice is anchored to the device origin rather than to the
// viewport, so it stays fixed wherever the blurred content sits.

struct BlurParams {
    // xy: the source texture's extent in texels. zw: the destination's.
    extents: vec4<f32>,
    // xy: this pass's direction, in source texels. z: the deviation, in source texels.
    // w: how many taps to take on each side of the centre.
    kernel: vec4<f32>,
    // x: how many texels of the source one device pixel covers. y: the same for the destination.
    // z: the spacing between taps, in source texels. w: non-zero when this pass halves resolution.
    sampling: vec4<f32>,
    // The part of the source this frame wrote, in source texels: the two corners. Reads are
    // clamped to it, so a partial redraw reads what a whole one does, and are faded out past it
    // when `edge.x` is set.
    valid: vec4<f32>,
    // xy: the device pixel the source's first texel holds. zw: the same for the destination.
    origins: vec4<f32>,
    // x: non-zero when the source is transparent outside `valid`. yzw: unused.
    edge: vec4<f32>,
}

@group(0) @binding(0) var<uniform> blur: BlurParams;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var source_sampler: sampler;

const BLUR_PI: f32 = 3.141592653589793;

struct BlurVarying {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vs_blur(@builtin(vertex_index) vertex: u32) -> BlurVarying {
    // A strip over the whole destination. The scissor restricts it to the region.
    let corner = vec2<f32>(f32(vertex & 1u), 0.5 * f32(vertex & 2u));
    var out: BlurVarying;
    out.position = vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
    return out;
}

// The device pixel a destination texel's centre sits at.
fn blur_device_position(position: vec2<f32>) -> vec2<f32> {
    return (floor(position) + vec2<f32>(0.5)) / blur.sampling.y + blur.origins.zw;
}

// Where a device position lands in the source, in source texels.
fn blur_source_texel(device: vec2<f32>) -> vec2<f32> {
    return (device - blur.origins.xy) * blur.sampling.x;
}

// The source at a position in source texels, clamped to what was written.
fn blur_sample(texel: vec2<f32>) -> vec4<f32> {
    let low = blur.valid.xy + vec2<f32>(0.5);
    let high = max(blur.valid.zw - vec2<f32>(0.5), low);
    let sampled =
        textureSampleLevel(source, source_sampler, clamp(texel, low, high) / blur.extents.xy, 0.0);
    if blur.edge.x == 0.0 {
        return sampled;
    }
    // The share of a bilinear tap that falls on written texels, so the rest reads as transparent.
    let inside = clamp(texel - blur.valid.xy + vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(1.0))
        * clamp(blur.valid.zw + vec2<f32>(0.5) - texel, vec2<f32>(0.0), vec2<f32>(1.0));
    return sampled * inside.x * inside.y;
}

@fragment
fn fs_blur_downsample(in: BlurVarying) -> @location(0) vec4<f32> {
    // A half-resolution texel's centre lies on the boundary of the two source texels it averages,
    // so one bilinear tap is the average.
    return blur_sample(blur_source_texel(blur_device_position(in.position.xy)));
}

@fragment
fn fs_blur_axis(in: BlurVarying) -> @location(0) vec4<f32> {
    let deviation = max(blur.kernel.z, 1e-4);
    let taps = i32(blur.kernel.w);
    let spacing = blur.sampling.z;
    let centre = blur_source_texel(blur_device_position(in.position.xy));
    let direction = blur.kernel.xy;

    var accumulated = vec4<f32>(0.0);
    var weight = 0.0;
    for (var tap = -taps; tap <= taps; tap = tap + 1) {
        let offset = f32(tap) * spacing;
        let contribution = gaussian_weight(offset, deviation);
        accumulated += blur_sample(centre + direction * offset) * contribution;
        weight += contribution;
    }
    return accumulated / max(weight, 1e-5);
}

fn gaussian_weight(distance: f32, deviation: f32) -> f32 {
    return exp(-(distance * distance) / (2.0 * deviation * deviation))
        / (sqrt(2.0 * BLUR_PI) * deviation);
}
