// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders.wgsl (Apache-2.0)
// The rounded-rectangle signed distance function, the quadrant-based corner selection and the
// half-pixel coverage rule are adapted from that work, which is licensed under the
// Apache License, Version 2.0, and have been modified: the scalar corner radius is generalised to
// a pair of elliptical semi-axes per corner with a superellipse exponent per corner, and the
// circular case is kept as the exact fast path.

// The corner radii of the quadrant the point lies in.
fn cr_pick_corner_radii(center_to_point: vec2<f32>, radii: cr_Radii) -> vec2<f32> {
    if center_to_point.x < 0.0 {
        if center_to_point.y < 0.0 {
            return vec2<f32>(radii.tl_x, radii.tl_y);
        }
        return vec2<f32>(radii.bl_x, radii.bl_y);
    }
    if center_to_point.y < 0.0 {
        return vec2<f32>(radii.tr_x, radii.tr_y);
    }
    return vec2<f32>(radii.br_x, radii.br_y);
}

// The corner exponent of the quadrant the point lies in.
fn cr_pick_corner_shape(center_to_point: vec2<f32>, shapes: cr_Shapes) -> f32 {
    if center_to_point.x < 0.0 {
        return select(shapes.bl, shapes.tl, center_to_point.y < 0.0);
    }
    return select(shapes.br, shapes.tr, center_to_point.y < 0.0);
}

// The exponent of an elliptical corner.
const cr_CORNER_ROUND: f32 = 2.0;

// Signed distance from a point mirrored into the bottom-right quadrant to a superellipse corner,
// negative inside, for any exponent but the ellipse's.
//
// `(f - 1) / |grad f|` with `f(p) = (|x/rx|^n + |y/ry|^n)^(1/n)` is a first-order distance.
fn cr_superellipse_corner_sdf(corner_center_to_point: vec2<f32>, r: vec2<f32>, n: f32) -> f32 {
    let power = clamp(n, 0.01, 64.0);
    // Away from zero, because the gradient raises this to `power - 1`, which can be negative.
    let normalised = max(abs(corner_center_to_point) / r, vec2<f32>(1.0e-6));
    let raised = pow(normalised, vec2<f32>(power));
    let sum = max(raised.x + raised.y, 1.0e-12);
    let value = pow(sum, 1.0 / power);
    let slope = pow(normalised, vec2<f32>(power - 1.0)) / r;
    let gradient = pow(sum, 1.0 / power - 1.0) * length(slope);
    return (value - 1.0) / max(gradient, 1.0e-6);
}

// Signed distance from a point mirrored into the bottom-right quadrant to the rounded boundary,
// negative inside. `r` holds the quadrant's semi-axes and `shape` its exponent.
fn cr_quad_sdf_impl(corner_center_to_point: vec2<f32>, r: vec2<f32>, shape: f32) -> f32 {
    let inside_corner = corner_center_to_point.x > 0.0 && corner_center_to_point.y > 0.0;
    if !inside_corner || r.x <= 0.0 || r.y <= 0.0 {
        return max(corner_center_to_point.x - r.x, corner_center_to_point.y - r.y);
    }
    // `pow(x, 2.0)` and `x * x` differ, so the ellipse keeps its own exact branch.
    if shape != cr_CORNER_ROUND {
        return cr_superellipse_corner_sdf(corner_center_to_point, r, shape);
    }
    if r.x == r.y {
        return length(corner_center_to_point) - r.x;
    }
    let k1 = length(corner_center_to_point / r);
    let k2 = length(corner_center_to_point / (r * r));
    return k1 * (k1 - 1.0) / k2;
}

// Signed distance from a point to a rounded rectangle, positive outside.
fn cr_quad_sdf(point: vec2<f32>, bounds: cr_Bounds, radii: cr_Radii, shapes: cr_Shapes) -> f32 {
    let half_size = cr_bounds_size(bounds) * 0.5;
    let center = cr_bounds_origin(bounds) + half_size;
    let center_to_point = point - center;
    let r = cr_pick_corner_radii(center_to_point, radii);
    let shape = cr_pick_corner_shape(center_to_point, shapes);
    let corner_to_point = abs(center_to_point) - half_size;
    return cr_quad_sdf_impl(corner_to_point + r, r, shape);
}

// The distance to the inner edge of a border corner, negative outside, positive inside.
fn cr_quarter_ellipse_sdf(point: vec2<f32>, radii: vec2<f32>, shape: f32) -> f32 {
    if radii.x <= 0.0 || radii.y <= 0.0 {
        return -max(point.x - radii.x, point.y - radii.y);
    }
    if shape != cr_CORNER_ROUND {
        return -cr_superellipse_corner_sdf(point, radii, shape);
    }
    let k1 = length(point / radii);
    let k2 = length(point / (radii * radii));
    return -(k1 * (k1 - 1.0) / k2);
}

// Coverage of a point by a rounded rectangle, with a band of half a unit on either side.
fn cr_rect_coverage(point: vec2<f32>, bounds: cr_Bounds, radii: cr_Radii, shapes: cr_Shapes) -> f32 {
    return saturate(0.5 - cr_quad_sdf(point, bounds, radii, shapes));
}

// Whether a point lies outside a box. Points on the edges are inside.
fn cr_outside_bounds(point: vec2<f32>, b: cr_Bounds) -> bool {
    return point.x < b.x || point.y < b.y || point.x > b.x + b.w || point.y > b.y + b.h;
}

// Coverage of a point by a rounded clip link: the rectangle's coverage, and zero outside the
// rectangle grown by the half-unit ramp, where an elliptical corner's distance can underestimate.
fn cr_clip_link_coverage(point: vec2<f32>, bounds: cr_Bounds, radii: cr_Radii, shapes: cr_Shapes) -> f32 {
    let grown = cr_Bounds(bounds.x - 0.5, bounds.y - 0.5, bounds.w + 1.0, bounds.h + 1.0);
    if cr_outside_bounds(point, grown) {
        return 0.0;
    }
    return cr_rect_coverage(point, bounds, radii, shapes);
}

// A general link spans five texels: the kind, space and mode; the rectangle; the radii; then the
// exponents of a rounded link or the mask cell of a mask link.
const cr_LINK_TEXELS: u32 = 5u;
const cr_LINK_ROUNDED: u32 = 0u;
const cr_LINK_ALPHA_MASK: u32 = 1u;
const cr_LINK_COLOR_MASK: u32 = 2u;
const cr_MASK_LUMINANCE: u32 = 1u;

// A mask texture sampled bilinearly at a texel position, never reading outside `cell`.
fn cr_mask_texel(masks: texture_2d<f32>, cell: vec4<f32>, texel: vec2<f32>) -> vec4<f32> {
    let low = cell.xy + vec2<f32>(0.5);
    let high = max(cell.xy + cell.zw - vec2<f32>(0.5), low);
    let at = clamp(texel, low, high) - vec2<f32>(0.5);
    let base = floor(at);
    let weight = at - base;
    let last = vec2<i32>(cell.xy + cell.zw) - vec2<i32>(1);
    let first = vec2<i32>(cell.xy);
    let a = vec2<i32>(base);
    let b = min(a + vec2<i32>(1), last);
    let c00 = textureLoad(masks, clamp(a, first, last), 0);
    let c10 = textureLoad(masks, vec2<i32>(b.x, a.y), 0);
    let c01 = textureLoad(masks, vec2<i32>(a.x, b.y), 0);
    let c11 = textureLoad(masks, b, 0);
    return mix(mix(c00, c10, weight.x), mix(c01, c11, weight.x), weight.y);
}

// Coverage of a device point by one general link, evaluated in the link's own space with its
// current matrix.
fn cr_link_coverage(device: vec2<f32>, index: u32) -> f32 {
    let base = index * cr_LINK_TEXELS;
    let head = textureLoad(cr_clip_links, cr_table_texel(base + 0u), 0);
    let r = cr_texel_f32(textureLoad(cr_clip_links, cr_table_texel(base + 1u), 0));
    let rect = cr_Bounds(r.x, r.y, r.z, r.w);
    let point = cr_space_point(device, head.y);
    let extra = cr_texel_f32(textureLoad(cr_clip_links, cr_table_texel(base + 4u), 0));
    if head.x == cr_LINK_ROUNDED {
        let lo = cr_texel_f32(textureLoad(cr_clip_links, cr_table_texel(base + 2u), 0));
        let hi = cr_texel_f32(textureLoad(cr_clip_links, cr_table_texel(base + 3u), 0));
        return cr_clip_link_coverage(point, rect, cr_radii_from_vec4(lo, hi), cr_shapes_from_vec4(extra));
    }
    if cr_outside_bounds(point, rect) {
        return 0.0;
    }
    let uv = (point - cr_bounds_origin(rect)) / max(cr_bounds_size(rect), vec2<f32>(1.0e-6));
    let texel = extra.xy + uv * extra.zw;
    if head.x == cr_LINK_ALPHA_MASK {
        return cr_mask_texel(cr_clip_alpha_masks, extra, texel).r;
    }
    let color = cr_mask_texel(cr_clip_color_masks, extra, texel);
    if head.z == cr_MASK_LUMINANCE {
        // The texels are premultiplied, so this is the luminance of the straight colour times alpha.
        return saturate(dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)));
    }
    return color.a;
}

// Coverage of a device point by a clip's general links, given as the first link's index plus one.
fn cr_links_coverage(device: vec2<f32>, links: u32) -> f32 {
    let start = links - 1u;
    let count = textureLoad(cr_clip_links, cr_table_texel(start * cr_LINK_TEXELS), 0).w;
    var coverage = 1.0;
    for (var i = 0u; i < count; i += 1u) {
        coverage *= cr_link_coverage(device, start + i);
        if coverage <= 0.0 {
            break;
        }
    }
    return coverage;
}

// Coverage of a device point by a whole clip. Every pipeline applies this function.
fn cr_clip_coverage(point: vec2<f32>, clip_id: u32) -> f32 {
    if clip_id == cr_NO_CLIP {
        return 1.0;
    }
    // The box holds the hard edges, which are not antialiased.
    if cr_outside_bounds(point, cr_clip_aabb(clip_id)) {
        return 0.0;
    }
    let scalars = cr_clip_scalars(clip_id);
    // The kept links are tested in their own space, which only translates.
    var first = point;
    var second = point;
    if scalars.y != cr_NO_SPACE {
        let kept = point - cr_load_spatial(scalars.y)[3].xy;
        if cr_outside_bounds(kept, cr_clip_kept_aabb(clip_id)) {
            return 0.0;
        }
        first = select(point, kept, (scalars.z & 1u) != 0u);
        second = select(point, kept, (scalars.z & 2u) != 0u);
    }
    var coverage = 1.0;
    if scalars.x > 0u {
        let test = cr_clip_rounded(clip_id, 0u);
        coverage *= cr_clip_link_coverage(first, test.rect, test.radii, test.shapes);
    }
    if scalars.x > 1u {
        let test = cr_clip_rounded(clip_id, 1u);
        coverage *= cr_clip_link_coverage(second, test.rect, test.radii, test.shapes);
    }
    if scalars.w != 0u && coverage > 0.0 {
        coverage *= cr_links_coverage(point, scalars.w);
    }
    return coverage;
}

// Coverage by a clip's device-space rounded tests and general links alone, for a caller that has
// already tested the box and knows the clip keeps no space.
fn cr_clip_rounded_coverage(point: vec2<f32>, clip_id: u32, count: u32, links: u32) -> f32 {
    var coverage = 1.0;
    if count > 0u {
        let first = cr_clip_rounded(clip_id, 0u);
        coverage = cr_clip_link_coverage(point, first.rect, first.radii, first.shapes);
    }
    if count > 1u {
        let second = cr_clip_rounded(clip_id, 1u);
        coverage *= cr_clip_link_coverage(point, second.rect, second.radii, second.shapes);
    }
    if links != 0u && coverage > 0.0 {
        coverage *= cr_links_coverage(point, links);
    }
    return coverage;
}

// The box a clip rejects outside of, or an unbounded box for no clip.
fn cr_clip_box(clip_id: u32) -> cr_Bounds {
    if clip_id == cr_NO_CLIP {
        return cr_Bounds(-1.0e9, -1.0e9, 2.0e9, 2.0e9);
    }
    return cr_clip_aabb(clip_id);
}

// The scalars of a clip, or none for no clip.
fn cr_clip_scalars_of(clip_id: u32) -> vec4<u32> {
    if clip_id == cr_NO_CLIP {
        return vec4<u32>(0u, cr_NO_SPACE, 0u, 0u);
    }
    return cr_clip_scalars(clip_id);
}

// Modulus with the sign of `a`.
fn cr_sdf_fmod(a: f32, b: f32) -> f32 {
    return a - b * trunc(a / b);
}
