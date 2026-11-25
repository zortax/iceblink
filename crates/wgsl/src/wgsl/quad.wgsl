// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders.wgsl (Apache-2.0)
// The quad fragment shader — the inner-edge signed distance, the border blend, and the dash
// parameterisation that lays dashes out clockwise around the whole perimeter — is adapted from
// that work, which is licensed under the Apache License, Version 2.0, and has been modified: every
// corner radius is a pair of elliptical semi-axes rather than a scalar, so each straight side takes
// the radii of its own axis, each quarter corner's arc length is a Ramanujan quarter-ellipse
// perimeter rather than `r * pi / 2`, and the position along a corner uses the eccentric anomaly.
// The background is a paint-table reference rather than an inline two-stop gradient, the clip is
// evaluated by a shared coverage function rather than four interpolated distances, each corner has
// its own superellipse exponent, and dotted borders are a second style.

struct Quad {
    bounds: cr_Bounds,
    radii: cr_Radii,
    shapes: cr_Shapes,
    border: cr_Edges,
    fill: cr_PaintRef,
    stroke: cr_PaintRef,
    // Subtracted from the point before either paint is evaluated.
    paint_origin: cr_Vector2,
    clip: u32,
    space: u32,
    style: u32,
}

@group(1) @binding(0) var quads: texture_2d<u32>;

// One `Quad`, which the arena pads to 8 texels.
fn load_quad(slot: u32) -> Quad {
    let base = slot * 8u;
    let t0 = cr_texel_f32(textureLoad(quads, cr_table_texel(base + 0u), 0));
    let t1 = cr_texel_f32(textureLoad(quads, cr_table_texel(base + 1u), 0));
    let t2 = cr_texel_f32(textureLoad(quads, cr_table_texel(base + 2u), 0));
    let t3 = cr_texel_f32(textureLoad(quads, cr_table_texel(base + 3u), 0));
    let t4 = cr_texel_f32(textureLoad(quads, cr_table_texel(base + 4u), 0));
    let t5 = textureLoad(quads, cr_table_texel(base + 5u), 0);
    let t6 = textureLoad(quads, cr_table_texel(base + 6u), 0);
    let t7 = textureLoad(quads, cr_table_texel(base + 7u), 0);
    return Quad(
        cr_Bounds(t0.x, t0.y, t0.z, t0.w),
        cr_Radii(t1.x, t1.y, t1.z, t1.w, t2.x, t2.y, t2.z, t2.w),
        cr_Shapes(t3.x, t3.y, t3.z, t3.w),
        cr_Edges(t4.x, t4.y, t4.z, t4.w),
        cr_PaintRef(cr_paint_kind(t5.x), t5.y),
        cr_PaintRef(cr_paint_kind(t5.z), t5.w),
        cr_Vector2(bitcast<f32>(t6.x), bitcast<f32>(t6.y)),
        t6.z,
        t6.w,
        t7.x & 0xffu,
    );
}

const BORDER_SOLID: u32 = 0u;
const BORDER_DASHED: u32 = 1u;
const BORDER_DOTTED: u32 = 2u;

struct QuadVarying {
    @builtin(position) position: vec4<f32>,
    // The point, from the quad's origin in the space it was encoded in.
    @location(0) local: vec2<f32>,
    // The quad's origin in that space.
    @location(1) @interpolate(flat) origin: vec2<f32>,
    // The instance's record, so the fragment stage does not load it again.
    @location(2) @interpolate(flat) v_bounds: vec4<f32>,
    @location(3) @interpolate(flat) v_radii_lo: vec4<f32>,
    @location(4) @interpolate(flat) v_radii_hi: vec4<f32>,
    @location(5) @interpolate(flat) v_border: vec4<f32>,
    @location(6) @interpolate(flat) v_paint_origin: vec2<f32>,
    @location(7) @interpolate(flat) v_refs: vec4<u32>,
    // The style, the clip, the clip's count of rounded tests, and its kept space.
    @location(8) @interpolate(flat) v_ids: vec4<u32>,
    // The clip's box, and the fill's colour where the fill is solid, read once per instance.
    @location(9) @interpolate(flat) v_clip_box: vec4<f32>,
    @location(10) @interpolate(flat) v_fill: vec4<f32>,
    @location(11) @interpolate(flat) v_shapes: vec4<f32>,
    // The clip's general links.
    @location(12) @interpolate(flat) v_links: u32,
}

@vertex
fn vs_quad(
    @builtin(vertex_index) vertex: u32,
    @location(0) slot: u32,
    @location(1) shift: vec2<f32>,
) -> QuadVarying {
    let quad = load_quad(slot);
    let local = cr_inflated_corner(vertex, quad.bounds) + shift;
    var out: QuadVarying;
    out.position = cr_to_clip_position(local, quad.space);
    out.local = cr_inflated_offset(vertex, quad.bounds);
    out.origin = cr_bounds_origin(quad.bounds);
    out.v_bounds = cr_bounds_of(quad.bounds);
    out.v_radii_lo = cr_radii_lo(quad.radii);
    out.v_radii_hi = cr_radii_hi(quad.radii);
    out.v_border = vec4<f32>(quad.border.top, quad.border.right, quad.border.bottom, quad.border.left);
    out.v_paint_origin = cr_vector2_of(quad.paint_origin);
    out.v_refs = vec4<u32>(quad.fill.kind, quad.fill.index, quad.stroke.kind, quad.stroke.index);
    let scalars = cr_clip_scalars_of(quad.clip);
    out.v_ids = vec4<u32>(quad.style, quad.clip, scalars.x, scalars.y);
    out.v_links = scalars.w;
    out.v_clip_box = cr_bounds_of(cr_clip_box(quad.clip));
    out.v_shapes = cr_shapes_of(quad.shapes);
    var fill = vec4<f32>(0.0);
    if quad.fill.kind == cr_PAINT_SOLID {
        fill = cr_rgba_of(cr_load_paint(quad.fill.index).color);
    }
    out.v_fill = fill;
    return out;
}

@fragment
fn fs_quad(in: QuadVarying) -> @location(0) vec4<f32> {
    // How long one device pixel is in the primitive's own space. Read before any branch, because
    // a derivative is only defined where every fragment of the quad takes it.
    let unit = max(max(fwidth(in.local).x, fwidth(in.local).y), 1e-6);
    let quad = Quad(
        cr_bounds_from_vec4(in.v_bounds),
        cr_radii_from_vec4(in.v_radii_lo, in.v_radii_hi),
        cr_shapes_from_vec4(in.v_shapes),
        cr_Edges(in.v_border.x, in.v_border.y, in.v_border.z, in.v_border.w),
        cr_PaintRef(in.v_refs.x, in.v_refs.y),
        cr_PaintRef(in.v_refs.z, in.v_refs.w),
        cr_Vector2(in.v_paint_origin.x, in.v_paint_origin.y),
        in.v_ids.y,
        0u,
        in.v_ids.x,
    );
    // The clip is evaluated at the device pixel and the shape at the point of the quad's space
    // that maps to it. A clip that is a box reads no table here.
    let at = cr_device_position(in.position.xy);
    if cr_outside_bounds(at, cr_bounds_from_vec4(in.v_clip_box)) {
        return vec4<f32>(0.0);
    }
    var clip = 1.0;
    if in.v_ids.w != cr_NO_SPACE {
        clip = cr_clip_coverage(at, quad.clip);
    } else if in.v_ids.z > 0u || in.v_links != 0u {
        clip = cr_clip_rounded_coverage(at, quad.clip, in.v_ids.z, in.v_links);
    }
    if clip <= 0.0 {
        return vec4<f32>(0.0);
    }
    let point = in.origin + in.local;
    let paint_origin = vec2<f32>(quad.paint_origin.x, quad.paint_origin.y);
    var background = in.v_fill;
    if quad.fill.kind != cr_PAINT_SOLID {
        background = cr_paint_color(quad.fill, point, paint_origin);
    }

    let size = cr_bounds_size(quad.bounds);
    let half_size = size * 0.5;
    let center_to_point = in.local - half_size;

    // The distances are in the quad's space, so every coverage is divided by the length of one
    // device pixel there.
    let antialias_threshold = 0.5 * unit;

    let corner_to_point = abs(center_to_point) - half_size;
    let corner_radii = cr_pick_corner_radii(center_to_point, quad.radii);
    let corner_shape = cr_pick_corner_shape(center_to_point, quad.shapes);
    let unrounded = quad.radii.tl_x == 0.0 && quad.radii.tl_y == 0.0
        && quad.radii.tr_x == 0.0 && quad.radii.tr_y == 0.0
        && quad.radii.br_x == 0.0 && quad.radii.br_y == 0.0
        && quad.radii.bl_x == 0.0 && quad.radii.bl_y == 0.0;
    let no_border = quad.border.top == 0.0 && quad.border.right == 0.0
        && quad.border.bottom == 0.0 && quad.border.left == 0.0;

    if unrounded && no_border {
        // Still weighted, because the quad is inflated by a unit on every side.
        let square_sdf = max(corner_to_point.x, corner_to_point.y);
        return background * saturate((antialias_threshold - square_sdf) / unit) * clip;
    }

    // The widths of the two nearest sides.
    let border = vec2<f32>(
        select(quad.border.right, quad.border.left, center_to_point.x < 0.0),
        select(quad.border.bottom, quad.border.top, center_to_point.y < 0.0),
    );
    // A zero-width side is pushed outside the antialiasing band so that no partial pixel is drawn
    // for a border that is not there.
    let reduced_border = vec2<f32>(
        select(border.x, -antialias_threshold, border.x == 0.0),
        select(border.y, -antialias_threshold, border.y == 0.0),
    );

    let corner_center_to_point = corner_to_point + corner_radii;
    let is_near_rounded_corner = corner_center_to_point.x >= 0.0 && corner_center_to_point.y >= 0.0;
    let straight_inner_corner_to_point = corner_to_point + reduced_border;
    // Past the antialiasing band of a straight inner edge. The inner corner lies inside those
    // edges, so a point this far beyond one is in the border whatever the corner's shape. Nearer
    // the edge, the curve of the inner corner decides.
    let is_beyond_inner_straight_border = straight_inner_corner_to_point.x > antialias_threshold
        || straight_inner_corner_to_point.y > antialias_threshold;
    // Inside both the inner edges and the outer edge's antialiasing band. A zero-width side puts
    // its inner edge outside the outer one, so the outer band is tested as well.
    let is_within_inner_straight_border =
        max(straight_inner_corner_to_point.x, corner_to_point.x) < -antialias_threshold
        && max(straight_inner_corner_to_point.y, corner_to_point.y) < -antialias_threshold;

    if is_within_inner_straight_border && !is_near_rounded_corner {
        return background * clip;
    }

    // Positive outside the outer edge of the border, negative inside it.
    let outer_sdf = cr_quad_sdf_impl(corner_center_to_point, corner_radii, corner_shape);

    // Positive inside the inner edge of the border, negative within the border itself.
    var inner_sdf = 0.0;
    if corner_center_to_point.x <= 0.0 || corner_center_to_point.y <= 0.0 {
        inner_sdf = -max(straight_inner_corner_to_point.x, straight_inner_corner_to_point.y);
    } else if is_beyond_inner_straight_border {
        inner_sdf = -1.0;
    } else if corner_shape == cr_CORNER_ROUND
        && reduced_border.x == reduced_border.y
        && corner_radii.x == corner_radii.y
    {
        // Circular inner edge: the outer distance shifted inwards is exact.
        inner_sdf = -(outer_sdf + reduced_border.x);
    } else {
        let ellipse_radii = max(vec2<f32>(0.0), corner_radii - reduced_border);
        inner_sdf = cr_quarter_ellipse_sdf(corner_center_to_point, ellipse_radii, corner_shape);
    }

    let border_sdf = max(inner_sdf, outer_sdf);

    var color = background;
    if border_sdf < antialias_threshold {
        var border_color: vec4<f32>;
        if quad.stroke.kind == cr_PAINT_SIDES {
            border_color = sides_color(quad.stroke, center_to_point, corner_to_point, border, unit);
        } else {
            border_color = cr_paint_color(quad.stroke, point, paint_origin);
        }
        let style = quad.style;
        if style != BORDER_SOLID {
            border_color *= dash_coverage(
                quad,
                style,
                point,
                center_to_point,
                corner_center_to_point,
                corner_radii,
                is_near_rounded_corner,
                unrounded,
                antialias_threshold,
            );
        }
        let blended = over_premultiplied(background, border_color);
        color = mix(background, blended, saturate((antialias_threshold - inner_sdf) / unit));
    }

    return color * saturate((antialias_threshold - outer_sdf) / unit) * clip;
}

// The colour of a border with one colour per side, at a point in the border.
//
// The sides meet on the line from the outer corner to the inner corner. A point
// belongs to the vertical side when `dx * bw_y < dy * bw_x`, where `dx` and `dy` are its distances
// in from the vertical and the horizontal edge and `bw` the two widths. A zero-width side never
// wins. The two colours are blended over one pixel across the diagonal.
fn sides_color(
    reference: cr_PaintRef,
    center_to_point: vec2<f32>,
    corner_to_point: vec2<f32>,
    widths: vec2<f32>,
    unit: f32,
) -> vec4<f32> {
    let paint = cr_load_paint(reference.index);
    let vertical = cr_side_color(paint, select(1u, 3u, center_to_point.x < 0.0));
    let horizontal = cr_side_color(paint, select(2u, 0u, center_to_point.y < 0.0));
    var toward_vertical = 0.0;
    if widths.x > 0.0 && widths.y > 0.0 {
        let inward = -corner_to_point;
        let across = (inward.y * widths.x - inward.x * widths.y) / length(widths);
        toward_vertical = saturate(0.5 + across / unit);
    } else if widths.x > 0.0 {
        toward_vertical = 1.0;
    }
    return mix(horizontal, vertical, toward_vertical);
}

// Premultiplied source-over.
fn over_premultiplied(below: vec4<f32>, above: vec4<f32>) -> vec4<f32> {
    return above + below * (1.0 - above.a);
}

// Ramanujan's approximation of a quarter-ellipse's arc length, which reduces to `r * pi / 2` when
// the two semi-axes are equal.
fn quarter_ellipse_perimeter(r: vec2<f32>) -> f32 {
    if r.x <= 0.0 && r.y <= 0.0 {
        return 0.0;
    }
    let h = ((r.x - r.y) * (r.x - r.y)) / max((r.x + r.y) * (r.x + r.y), 1e-6);
    return 0.25 * cr_M_PI * (r.x + r.y) * (1.0 + (3.0 * h) / (10.0 + sqrt(max(4.0 - 3.0 * h, 0.0))));
}

// The slower of two dash velocities, so a corner takes the larger dashes of the two sides it
// joins. A zero velocity means a zero-width side, which contributes nothing.
fn corner_dash_velocity(first: f32, second: f32) -> f32 {
    if first == 0.0 {
        return second;
    }
    if second == 0.0 {
        return first;
    }
    return min(first, second);
}

// Coverage of a dash at position `t` in dash space, where one dash period has length one.
fn dash_alpha(t: f32, period: f32, length: f32, velocity: f32, threshold: f32) -> f32 {
    let half_period = period * 0.5;
    let half_length = length * 0.5;
    let centered = cr_sdf_fmod(t + half_period - half_length, period) - half_period;
    let signed_distance = abs(centered) - half_length;
    return saturate(threshold - signed_distance / max(velocity, 1e-6));
}

// How much of this pixel a dashed or dotted border covers.
//
// Dash size is proportional to border width, which keeps dashes from
// overlapping where the border is thicker than the dash. A dotted border is the same machinery
// with a one-to-one dash and gap.
fn dash_coverage(
    quad: Quad,
    style: u32,
    point: vec2<f32>,
    center_to_point: vec2<f32>,
    corner_center_to_point: vec2<f32>,
    corner_radii: vec2<f32>,
    is_near_rounded_corner: bool,
    unrounded: bool,
    threshold: f32,
) -> f32 {
    let dash_per_width = select(2.0, 1.0, style == BORDER_DOTTED);
    let gap_per_width = 1.0;
    let period_per_width = dash_per_width + gap_per_width;
    let dv_numerator = 1.0 / period_per_width;

    let size = cr_bounds_size(quad.bounds);
    let origin = cr_bounds_origin(quad.bounds);
    let local = point - origin;

    var t = 0.0;
    var max_t = 0.0;
    var velocity = 0.0;

    if unrounded {
        // Without rounded corners each side lays its dashes out on its own, so every side starts
        // and ends with a dash.
        let is_horizontal = corner_center_to_point.x < corner_center_to_point.y;
        let widths = vec2<f32>(
            max(quad.border.bottom, quad.border.top),
            max(quad.border.right, quad.border.left),
        );
        let width = select(widths.y, widths.x, is_horizontal);
        velocity = select(0.0, dv_numerator / width, width > 0.0);
        t = select(local.y, local.x, is_horizontal) * velocity;
        max_t = select(size.y, size.x, is_horizontal) * velocity;
    } else {
        let r_tl = vec2<f32>(quad.radii.tl_x, quad.radii.tl_y);
        let r_tr = vec2<f32>(quad.radii.tr_x, quad.radii.tr_y);
        let r_br = vec2<f32>(quad.radii.br_x, quad.radii.br_y);
        let r_bl = vec2<f32>(quad.radii.bl_x, quad.radii.bl_y);

        let dv_t = select(0.0, dv_numerator / quad.border.top, quad.border.top > 0.0);
        let dv_r = select(0.0, dv_numerator / quad.border.right, quad.border.right > 0.0);
        let dv_b = select(0.0, dv_numerator / quad.border.bottom, quad.border.bottom > 0.0);
        let dv_l = select(0.0, dv_numerator / quad.border.left, quad.border.left > 0.0);

        // A straight side runs between the two corners on its own axis, so it takes the x radii of
        // the horizontal sides and the y radii of the vertical ones.
        let s_t = max(size.x - r_tl.x - r_tr.x, 0.0) * dv_t;
        let s_r = max(size.y - r_tr.y - r_br.y, 0.0) * dv_r;
        let s_b = max(size.x - r_br.x - r_bl.x, 0.0) * dv_b;
        let s_l = max(size.y - r_bl.y - r_tl.y, 0.0) * dv_l;

        let cv_tr = corner_dash_velocity(dv_t, dv_r);
        let cv_br = corner_dash_velocity(dv_b, dv_r);
        let cv_bl = corner_dash_velocity(dv_b, dv_l);
        let cv_tl = corner_dash_velocity(dv_t, dv_l);

        let c_tr = quarter_ellipse_perimeter(r_tr) * cv_tr;
        let c_br = quarter_ellipse_perimeter(r_br) * cv_br;
        let c_bl = quarter_ellipse_perimeter(r_bl) * cv_bl;
        let c_tl = quarter_ellipse_perimeter(r_tl) * cv_tl;

        let upto_tr = s_t;
        let upto_r = upto_tr + c_tr;
        let upto_br = upto_r + s_r;
        let upto_b = upto_br + c_br;
        let upto_bl = upto_b + s_b;
        let upto_l = upto_bl + c_bl;
        let upto_tl = upto_l + s_l;
        max_t = upto_tl + c_tl;

        if is_near_rounded_corner {
            // The eccentric anomaly, scaled by the corner's own arc length, keeps the dash rhythm
            // continuous across an elliptical corner. The angle alone would run fast on the short
            // axis and slow on the long one.
            let radii = max(corner_radii, vec2<f32>(1e-6));
            let anomaly = atan2(corner_center_to_point.y / radii.y, corner_center_to_point.x / radii.x);
            let fraction = saturate(anomaly / (0.5 * cr_M_PI));

            if center_to_point.x >= 0.0 {
                if center_to_point.y < 0.0 {
                    velocity = cv_tr;
                    t = upto_r - fraction * c_tr;
                } else {
                    velocity = cv_br;
                    t = upto_br + fraction * c_br;
                }
            } else {
                if center_to_point.y >= 0.0 {
                    velocity = cv_bl;
                    t = upto_l - fraction * c_bl;
                } else {
                    velocity = cv_tl;
                    t = upto_tl + fraction * c_tl;
                }
            }
        } else {
            let is_horizontal = corner_center_to_point.x < corner_center_to_point.y;
            if is_horizontal {
                if center_to_point.y < 0.0 {
                    velocity = dv_t;
                    t = (local.x - r_tl.x) * velocity;
                } else {
                    velocity = dv_b;
                    t = upto_bl - (local.x - r_bl.x) * velocity;
                }
            } else {
                if center_to_point.x < 0.0 {
                    velocity = dv_l;
                    t = upto_tl - (local.y - r_tl.y) * velocity;
                } else {
                    velocity = dv_r;
                    t = upto_r + (local.y - r_tr.y) * velocity;
                }
            }
        }
    }

    let dash_length = dash_per_width / period_per_width;
    // Shortening a straight run by one dash before dividing makes it start and end with a dash.
    max_t -= select(0.0, dash_length, unrounded);
    if max_t >= 1.0 {
        let dash_count = floor(max_t);
        return dash_alpha(t, max_t / dash_count, dash_length, velocity, threshold);
    }
    if unrounded {
        let dash_gap = max_t - dash_length;
        if dash_gap > 0.0 {
            return dash_alpha(t, dash_length + dash_gap, dash_length, velocity, threshold);
        }
    }
    return 1.0;
}
