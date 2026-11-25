//! Path items, each drawn as path masks or in a vector pass of its own.

mod mask;

use std::sync::Arc;

use iceblink::kurbo::{Affine, BezPath, Cap, Join, PathEl, Point, Rect, Stroke};
use iceblink::{
    BatchKind, DeviceRect, FRAME_NO_CLIP, FramePath, Matrix4, PassKey, PathClip, PathFill,
    PathItem, PathStroke, PlannedPass, PlannedPath,
};

use crate::naive::content::PathId;
use crate::naive::frame::builder::Builder;
use crate::naive::frame::item::{self, Placed};
use crate::naive::geometry::{Bounds, device_rect, map_point, plane_affine, translation};
use crate::naive::scene::Chunk;

use self::mask::MaskPlan;

/// Draws path item `id` as path masks when it qualifies and its route history admits them, and as
/// a frame path otherwise, and returns its device ink.
pub(crate) fn path(
    builder: &mut Builder<'_>,
    chunk: &Chunk,
    item: &PathItem,
    id: PathId,
) -> Bounds {
    let Some(placed) = item::place(builder, chunk, item.space, item.clip) else {
        return Bounds::EMPTY;
    };

    let reach = item
        .stroke
        .as_ref()
        .map_or(0.0, |stroke| reach(&stroke.style));
    let mut local = to_bounds(
        item.placement
            .transform_rect_bbox(item.path.control_box().inflate(reach, reach)),
    );
    for clip in item.clips.iter() {
        local = local.intersect(to_bounds(
            item.placement.transform_rect_bbox(clip.path.control_box()),
        ));
    }

    let ink = placed.ink(local, builder.env.output);
    if !item::shows(builder, ink) {
        return ink;
    }

    let fill = item.fill.map(|fill| PathFill {
        paint: builder.paint(fill.paint),
        rule: fill.rule,
    });
    let stroke = item.stroke.as_ref().map(|stroke| PathStroke {
        paint: builder.paint(stroke.paint),
        style: stroke.style.clone(),
    });

    if let Some(plan) = MaskPlan::new(item, fill, stroke.as_ref(), &placed)
        && builder.content.routes.route(id, plan.masks(), &item.path)
        && plan.draw(builder, &placed)
    {
        return ink;
    }

    let local_to_device = placed.world * translation(placed.offset);
    let Some(mapped) = Mapped::new(&local_to_device, item.placement, &item.path, &item.clips)
    else {
        return ink;
    };

    let path = FramePath {
        path: mapped.path,
        transform: mapped.transform,
        brush: item.brush,
        fill,
        stroke,
        clips: mapped.clips,
        ink: ink.round_out().expect("a shown ink is not empty"),
    };
    push(builder, &placed, path);
    ink
}

/// A path's outline and clips with the matrix that maps them to the device.
pub(crate) struct Mapped {
    pub(crate) path: Arc<BezPath>,
    pub(crate) transform: Affine,
    pub(crate) clips: Arc<[PathClip]>,
}

impl Mapped {
    /// Maps path space to the device through `placement` and then `world`.
    ///
    /// Under an affine world matrix the outlines are kept and the matrix is the transform. Under
    /// a projective one every point is mapped and the transform is the identity. Returns `None`
    /// when a point maps to a `w` of zero or below.
    pub(crate) fn new(
        world: &Matrix4,
        placement: Affine,
        path: &Arc<BezPath>,
        clips: &Arc<[PathClip]>,
    ) -> Option<Self> {
        if let Some(affine) = plane_affine(world) {
            return Some(Self {
                path: path.clone(),
                transform: affine * placement,
                clips: clips.clone(),
            });
        }

        let project = |outline: &BezPath| project(world, placement, outline);
        let clips = clips
            .iter()
            .map(|clip| {
                Some(PathClip {
                    path: Arc::new(project(&clip.path)?),
                    rule: clip.rule,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            path: Arc::new(project(path)?),
            transform: Affine::IDENTITY,
            clips: clips.into(),
        })
    }
}

/// Maps every point of `outline` through `placement` and then the projective `world`.
fn project(world: &Matrix4, placement: Affine, outline: &BezPath) -> Option<BezPath> {
    let map = |point: Point| -> Option<Point> {
        let point = placement * point;
        let [x, y] = map_point(world, point.x as f32, point.y as f32)?;
        Some(Point::new(f64::from(x), f64::from(y)))
    };

    let mut projected = BezPath::new();
    for element in outline.elements() {
        projected.push(match *element {
            PathEl::MoveTo(p) => PathEl::MoveTo(map(p)?),
            PathEl::LineTo(p) => PathEl::LineTo(map(p)?),
            PathEl::QuadTo(a, b) => PathEl::QuadTo(map(a)?, map(b)?),
            PathEl::CurveTo(a, b, c) => PathEl::CurveTo(map(a)?, map(b)?, map(c)?),
            PathEl::ClosePath => PathEl::ClosePath,
        });
    }
    Some(projected)
}

/// Appends a frame path and plans a vector pass of its own for it.
///
/// The pass region is the part of the path's ink on the output, aligned outwards to multiples of
/// 16 and cut to the output. The pass clip is the path's whole chain, so no residual is left. The
/// pass overhangs when the path's outline reaches past the region.
pub(crate) fn push(builder: &mut Builder<'_>, placed: &Placed, path: FramePath) {
    let output = builder.output_rect();
    let ink = path.ink;
    // Pass regions lie on the output, so a path wholly past it inside a group is left out.
    let Some(shown) = ink.intersection(output) else {
        return;
    };
    let region = aligned(shown)
        .intersection(output)
        .expect("aligned contains shown");
    let overhangs = !inside(&path, region);

    let index = builder.frame.paths.len() as u32;
    builder.frame.paths.push(path);

    let plan = &mut builder.frame.vector_plan;
    let first = plan.items.len() as u32;
    plan.items.push(PlannedPath {
        path: index,
        residual: FRAME_NO_CLIP,
        clip: placed.clip,
        ink: device_rect(
            ink.origin.x - region.origin.x,
            ink.origin.y - region.origin.y,
            ink.size.width,
            ink.size.height,
        ),
    });

    let pass = plan.passes.len() as u32;
    plan.passes.push(PlannedPass {
        items: first..first + 1,
        region,
        clip: placed.clip,
        instanced: true,
        clamped: false,
        overhangs,
        key: PassKey::NONE,
    });

    builder.counters.items_drawn += 1;
    builder.counters.vector_passes += 1;
    builder.batch(BatchKind::Paths { pass });
}

/// Reports whether the outline of `path`, its stroke included, lies inside `region`.
fn inside(path: &FramePath, region: DeviceRect) -> bool {
    let reach = path
        .stroke
        .as_ref()
        .map_or(0.0, |stroke| reach(&stroke.style));
    let outline = path
        .transform
        .transform_rect_bbox(path.path.control_box().inflate(reach, reach));
    outline.x0 >= f64::from(region.left())
        && outline.y0 >= f64::from(region.top())
        && outline.x1 <= f64::from(region.right())
        && outline.y1 <= f64::from(region.bottom())
}

/// Returns `rect` with its edges moved outwards to multiples of 16.
fn aligned(rect: DeviceRect) -> DeviceRect {
    let x0 = rect.origin.x.div_euclid(16) * 16;
    let y0 = rect.origin.y.div_euclid(16) * 16;
    let x1 = (rect.origin.x + rect.size.width + 15).div_euclid(16) * 16;
    let y1 = (rect.origin.y + rect.size.height + 15).div_euclid(16) * 16;
    device_rect(x0, y0, x1 - x0, y1 - y0)
}

/// Returns how far a stroke reaches past the outline: half its width, times the miter limit for
/// a miter join and √2 for a square cap, whichever is largest.
pub(crate) fn reach(stroke: &Stroke) -> f64 {
    let miter = if stroke.join == Join::Miter {
        stroke.miter_limit
    } else {
        1.0
    };
    let square = if stroke.start_cap == Cap::Square || stroke.end_cap == Cap::Square {
        std::f64::consts::SQRT_2
    } else {
        1.0
    };
    stroke.width / 2.0 * 1.0f64.max(miter).max(square)
}

pub(crate) fn to_bounds(rect: Rect) -> Bounds {
    Bounds {
        x0: rect.x0 as f32,
        y0: rect.y0 as f32,
        x1: rect.x1 as f32,
        y1: rect.y1 as f32,
    }
}
