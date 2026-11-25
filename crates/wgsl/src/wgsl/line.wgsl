// Decoration lines: underline, overline and line-through, in five styles.

struct Line {
    bounds: cr_Bounds,
    color: cr_Rgba,
    thickness: f32,
    clip: u32,
    space: u32,
    style: u32,
}

@group(1) @binding(0) var lines: texture_2d<u32>;

// One `Line`, which is 3 texels.
fn load_line(slot: u32) -> Line {
    let base = slot * 3u;
    let t0 = cr_texel_f32(textureLoad(lines, cr_table_texel(base + 0u), 0));
    let t1 = cr_texel_f32(textureLoad(lines, cr_table_texel(base + 1u), 0));
    let t2 = textureLoad(lines, cr_table_texel(base + 2u), 0);
    return Line(
        cr_Bounds(t0.x, t0.y, t0.z, t0.w),
        cr_Rgba(t1.x, t1.y, t1.z, t1.w),
        bitcast<f32>(t2.x),
        t2.y,
        t2.z,
        t2.w & 0xffu,
    );
}

const LINE_SOLID: u32 = 0u;
const LINE_WAVY: u32 = 1u;
const LINE_DASHED: u32 = 2u;
const LINE_DOTTED: u32 = 3u;
const LINE_DOUBLE: u32 = 4u;

struct LineVarying {
    @builtin(position) position: vec4<f32>,
    // The point, from the line's origin.
    @location(0) local: vec2<f32>,
    // The instance's record, so the fragment stage does not load it again.
    @location(1) @interpolate(flat) v_bounds: vec4<f32>,
    @location(2) @interpolate(flat) v_color: vec4<f32>,
    @location(3) @interpolate(flat) v_ids: vec2<u32>,
    @location(4) @interpolate(flat) v_thickness: f32,
}

@vertex
fn vs_line(
    @builtin(vertex_index) vertex: u32,
    @location(0) slot: u32,
    @location(1) shift: vec2<f32>,
) -> LineVarying {
    let line = load_line(slot);
    let local = cr_inflated_corner(vertex, line.bounds) + shift;
    var out: LineVarying;
    out.position = cr_to_clip_position(local, line.space);
    out.local = cr_inflated_offset(vertex, line.bounds);
    out.v_bounds = cr_bounds_of(line.bounds);
    out.v_color = cr_rgba_of(line.color);
    out.v_ids = vec2<u32>(line.clip, line.style);
    out.v_thickness = line.thickness;
    return out;
}

@fragment
fn fs_line(in: LineVarying) -> @location(0) vec4<f32> {
    let clip = cr_clip_coverage(cr_device_position(in.position.xy), in.v_ids.x);
    if clip <= 0.0 {
        return vec4<f32>(0.0);
    }

    let size = cr_bounds_size(cr_bounds_from_vec4(in.v_bounds));
    let local = in.local;
    let thickness = max(in.v_thickness, 1.0);
    let style = in.v_ids.y;

    var coverage = 0.0;
    if style == LINE_WAVY {
        let amplitude = max((size.y - thickness) * 0.5, 0.0);
        let wavelength = max(thickness * 4.0, 1.0);
        let middle = size.y * 0.5;
        let wave = middle + amplitude * sin(local.x * 2.0 * cr_M_PI / wavelength);
        let ends = horizontal_extent(local, size) * vertical_extent(local, size);
        coverage = saturate(0.5 + thickness * 0.5 - abs(local.y - wave)) * ends;
    } else if style == LINE_DOUBLE {
        let band = size.y / 3.0;
        let first = saturate(0.5 + band * 0.5 - abs(local.y - band * 0.5));
        let second = saturate(0.5 + band * 0.5 - abs(local.y - band * 2.5));
        coverage = max(first, second) * horizontal_extent(local, size) * vertical_extent(local, size);
    } else {
        coverage = horizontal_extent(local, size) * vertical_extent(local, size);
        if style == LINE_DASHED || style == LINE_DOTTED {
            let dash = select(thickness * 3.0, thickness, style == LINE_DOTTED);
            let period = dash * 2.0;
            let phase = cr_sdf_fmod(local.x, period);
            coverage *= saturate(0.5 + dash - abs(phase - dash * 0.5) - dash * 0.5);
        }
    }

    return in.v_color * coverage * clip;
}

// Coverage along the line, antialiased at both ends.
fn horizontal_extent(local: vec2<f32>, size: vec2<f32>) -> f32 {
    return saturate(0.5 + min(local.x, size.x - local.x));
}

// Coverage across the line, antialiased on both edges.
fn vertical_extent(local: vec2<f32>, size: vec2<f32>) -> f32 {
    return saturate(0.5 + min(local.y, size.y - local.y));
}
