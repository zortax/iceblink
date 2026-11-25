// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders.wgsl (Apache-2.0)
// Ported from iceblink-wgsl's `quad.wgsl`. The inner-edge signed distance, the border blend and the
// dash parameterisation that lays dashes out clockwise around the whole perimeter are adapted from
// that work, which is licensed under the Apache License, Version 2.0, and have been modified: every
// corner radius is a pair of elliptical semi-axes, each quarter corner's arc length is a Ramanujan
// quarter-ellipse perimeter, the position along a corner uses the eccentric anomaly, each corner
// has its own superellipse exponent, and dotted borders are a second style.

//! Quads: a rounded rectangle with a fill and a border, solid, dashed or dotted.

mod dash;

use iceblink::geom::{Device, Point, Rect};
use iceblink::{BorderStyle, PaintKind, Quad};

use crate::draw::footprint::{footprint, holds};
use crate::draw::{Scene, paint_around};
use crate::math::saturate;
use crate::math::sdf::{
    CORNER_ROUND, pick_corner_radii, pick_corner_shape, quad_sdf_impl, quarter_ellipse_sdf,
};
use crate::math::vec::{length, mix4, over, scale4};
use crate::tables::{Clip, Painter, Space};
use crate::target::Surface;

/// Draws `quads` in order into `surface`, cut to `scissor`.
pub(crate) fn draw_quads(
    scene: &Scene<'_>,
    surface: &mut Surface<'_>,
    scissor: Rect<i32, Device>,
    quads: &[Quad],
    row: &mut Vec<[f32; 4]>,
) {
    let Some(within) = scissor.intersection(surface.extent()) else {
        return;
    };
    for quad in quads {
        let inflated = inflate(quad.bounds);
        let space = scene.space(quad.space.0);
        let Some(area) = footprint(space, inflated, within) else {
            continue;
        };
        let clip = scene.clip(quad.clip.0);
        let fill = Painter::of(scene.paints, quad.fill);
        let stroke = Painter::of(scene.paints, quad.stroke);
        let core = core(quad, space, clip, &fill);
        paint_around(surface, area, core, row, |device| {
            let Some(point) = space.local(device) else {
                return [0.0; 4];
            };
            if !holds(inflated, point) {
                return [0.0; 4];
            }
            let coverage = scene.clip_coverage(clip, device);
            if coverage <= 0.0 {
                return [0.0; 4];
            }
            let [dx, dy] = space.steps(device);
            let unit = (dx[0].abs() + dy[0].abs())
                .max(dx[1].abs() + dy[1].abs())
                .max(1e-6);
            scale4(shade(quad, &fill, &stroke, point, unit), coverage)
        });
    }
}

// The device pixels where a quad paints its solid fill alone, and that colour. They lie at least
// half a pixel inside every border and corner, and there are none unless the space only
// translates, the fill is solid and the clip is a box.
fn core(
    quad: &Quad,
    space: &Space,
    clip: Option<&Clip>,
    fill: &Painter<'_>,
) -> Option<(Rect<i32, Device>, [f32; 4])> {
    let [tx, ty] = space.translation()?;
    let color = fill.solid()?;
    let border = quad
        .border
        .iter()
        .fold(0.0f32, |held, width| held.max(*width));
    let radius = quad
        .radii
        .iter()
        .fold(0.0f32, |held, radius| held.max(*radius));
    let inset = (border + 0.5).max(radius) + 1.0;
    let [x, y, width, height] = quad.bounds;
    let mut low = [x + tx + inset, y + ty + inset];
    let mut high = [x + width + tx - inset, y + height + ty - inset];
    if let Some(clip) = clip {
        let [cx, cy, cw, ch] = clip.box_only()?;
        low = [low[0].max(cx), low[1].max(cy)];
        high = [high[0].min(cx + cw), high[1].min(cy + ch)];
    }
    // The pixels whose centres lie in the box.
    let first = low.map(|value| (value - 0.5).ceil() as i32);
    let last = high.map(|value| (value - 0.5).floor() as i32);
    (last[0] >= first[0] && last[1] >= first[1]).then(|| {
        (
            Rect::from_corners(
                Point::new(first[0], first[1]),
                Point::new(last[0] + 1, last[1] + 1),
            ),
            color,
        )
    })
}

/// Returns `bounds` grown by one unit on every side, the area a quad is rasterised over.
pub(crate) fn inflate(bounds: [f32; 4]) -> [f32; 4] {
    [
        bounds[0] - 1.0,
        bounds[1] - 1.0,
        bounds[2] + 2.0,
        bounds[3] + 2.0,
    ]
}

// The quad's colour at the local point `point`, before its clip.
fn shade(
    quad: &Quad,
    fill: &Painter<'_>,
    stroke: &Painter<'_>,
    point: [f32; 2],
    unit: f32,
) -> [f32; 4] {
    let background = fill.at(point, quad.paint_origin);
    let local = [point[0] - quad.bounds[0], point[1] - quad.bounds[1]];
    let half_size = [quad.bounds[2] * 0.5, quad.bounds[3] * 0.5];
    let center_to_point = [local[0] - half_size[0], local[1] - half_size[1]];
    let threshold = 0.5 * unit;

    let corner_to_point = [
        center_to_point[0].abs() - half_size[0],
        center_to_point[1].abs() - half_size[1],
    ];
    let corner_radii = pick_corner_radii(center_to_point, &quad.radii);
    let corner_shape = pick_corner_shape(center_to_point, &quad.shapes);
    let unrounded = quad.radii.iter().all(|radius| *radius == 0.0);
    let no_border = quad.border.iter().all(|width| *width == 0.0);

    if unrounded && no_border {
        // Still weighted, because the quad is inflated by a unit on every side.
        let square = corner_to_point[0].max(corner_to_point[1]);
        return scale4(background, saturate((threshold - square) / unit));
    }

    let [top, right, bottom, left] = quad.border;
    let border = [
        if center_to_point[0] < 0.0 {
            left
        } else {
            right
        },
        if center_to_point[1] < 0.0 {
            top
        } else {
            bottom
        },
    ];
    // A zero-width side is pushed outside the antialiasing band.
    let reduced = border.map(|width| if width == 0.0 { -threshold } else { width });

    let corner_center = [
        corner_to_point[0] + corner_radii[0],
        corner_to_point[1] + corner_radii[1],
    ];
    let near_rounded = corner_center[0] >= 0.0 && corner_center[1] >= 0.0;
    let straight_inner = [
        corner_to_point[0] + reduced[0],
        corner_to_point[1] + reduced[1],
    ];
    let beyond_inner = straight_inner[0] > threshold || straight_inner[1] > threshold;
    let within_inner = straight_inner[0].max(corner_to_point[0]) < -threshold
        && straight_inner[1].max(corner_to_point[1]) < -threshold;
    if within_inner && !near_rounded {
        return background;
    }

    let outer = quad_sdf_impl(corner_center, corner_radii, corner_shape);
    let inner = if corner_center[0] <= 0.0 || corner_center[1] <= 0.0 {
        -straight_inner[0].max(straight_inner[1])
    } else if beyond_inner {
        -1.0
    } else if corner_shape == CORNER_ROUND
        && reduced[0] == reduced[1]
        && corner_radii[0] == corner_radii[1]
    {
        -(outer + reduced[0])
    } else {
        let radii = [
            (corner_radii[0] - reduced[0]).max(0.0),
            (corner_radii[1] - reduced[1]).max(0.0),
        ];
        quarter_ellipse_sdf(corner_center, radii, corner_shape)
    };

    let border_sdf = inner.max(outer);
    let mut color = background;
    if border_sdf < threshold {
        let mut border_color = if quad.stroke.kind == PaintKind::SIDES {
            sides(stroke, center_to_point, corner_to_point, border, unit)
        } else {
            stroke.at(point, quad.paint_origin)
        };
        if quad.style != BorderStyle::SOLID {
            let spot = dash::Spot {
                point,
                center_to_point,
                corner_center,
                corner_radii,
                near_rounded,
                unrounded,
                threshold,
            };
            let dash = dash::coverage(quad, &spot);
            border_color = scale4(border_color, dash);
        }
        let blended = over(background, border_color);
        color = mix4(background, blended, saturate((threshold - inner) / unit));
    }
    scale4(color, saturate((threshold - outer) / unit))
}

// The colour of a border with one colour per side. A point belongs to the vertical side when
// `dx × w_y < dy × w_x`, and the two colours blend over one pixel across the diagonal.
fn sides(
    stroke: &Painter<'_>,
    center_to_point: [f32; 2],
    corner_to_point: [f32; 2],
    widths: [f32; 2],
    unit: f32,
) -> [f32; 4] {
    let vertical = stroke.side(if center_to_point[0] < 0.0 { 3 } else { 1 });
    let horizontal = stroke.side(if center_to_point[1] < 0.0 { 0 } else { 2 });
    let toward_vertical = if widths[0] > 0.0 && widths[1] > 0.0 {
        let inward = [-corner_to_point[0], -corner_to_point[1]];
        let across = (inward[1] * widths[0] - inward[0] * widths[1]) / length(widths);
        saturate(0.5 + across / unit)
    } else if widths[0] > 0.0 {
        1.0
    } else {
        0.0
    };
    mix4(horizontal, vertical, toward_vertical)
}
