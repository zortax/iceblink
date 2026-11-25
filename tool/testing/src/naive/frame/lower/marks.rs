//! Marks items, and the placements of oversize shapes drawn as paths.

use std::sync::Arc;

use iceblink::kurbo::{Affine, BezPath, Rect, Shape};
use iceblink::{
    BatchKind, ClipRef, FillRule, FrameMarkPayload, FrameMarks, FramePath, Instance, MarkCell,
    MarkFlags, MarkPayload, Marks, Matrix4, PaintRef, PathClip, PathFill, SpaceRef,
};

use crate::naive::content::{Content, MarkCells};
use crate::naive::frame::builder::Builder;
use crate::naive::frame::item::{self, Placed};
use crate::naive::frame::lower::path;
use crate::naive::geometry::{Bounds, map_point, plane_affine, translation};
use crate::naive::scene::Chunk;

pub(crate) fn marks(builder: &mut Builder<'_>, chunk: &Chunk, marks: &Marks) -> Bounds {
    let Some(placed) = item::place(builder, chunk, marks.space, marks.clip) else {
        return Bounds::EMPTY;
    };

    let ink = placed.ink(Bounds::of(marks.bounds), builder.env.output);
    let Some(payload) = chunk.recorded.payloads.get(marks.payload as usize) else {
        return ink;
    };
    if !item::shows(builder, ink) {
        return ink;
    }

    let cells = cells(builder, payload, marks.placements > 0);
    let paint = builder.paint(marks.paint);
    let index = payload_index(builder, payload, &cells);
    if let Some(page) = cells.page {
        builder.pages.insert(page);
    }

    let mut lowered = *marks;
    lowered.paint = paint;
    lowered.payload = index;
    lowered.clip = ClipRef(placed.clip);
    lowered.space = SpaceRef(placed.space);
    let instance = FrameMarks {
        marks: lowered,
        page: cells.page.unwrap_or(0),
    }
    .moved(placed.offset);

    let position = builder.frame.lanes.marks.sources.len() as u32;
    builder.instance(|lanes| &mut lanes.marks, instance, BatchKind::Marks);
    if marks.flags.contains(MarkFlags::UNION) {
        builder
            .union_marks
            .push((position, ink.round_out().expect("a shown ink is not empty")));
    }

    oversize(builder, &placed, marks, payload, &cells, paint);
    ink
}

/// Returns the cells of a payload, rasterised only when the item draws placements.
fn cells(builder: &mut Builder<'_>, payload: &Arc<MarkPayload>, placements: bool) -> MarkCells {
    if !placements {
        let count = payload.shapes.len();
        return MarkCells {
            page: None,
            cells: vec![MarkCell::default(); count * 16].into(),
            oversize: vec![false; count].into(),
        };
    }

    let side = Content::shared_side(builder.env.caps.max_texture_size);
    let content = &mut *builder.content;
    content.marks.cells(&mut content.pages, side, payload)
}

/// Returns the index in `Lanes::mark_payloads` of a payload with `cells`, adding it the first time
/// the frame draws it.
fn payload_index(builder: &mut Builder<'_>, payload: &Arc<MarkPayload>, cells: &MarkCells) -> u32 {
    let key = (
        Arc::as_ptr(payload) as usize,
        Arc::as_ptr(&cells.cells) as *const u8 as usize,
    );
    if let Some(index) = builder.mark_payloads.get(&key) {
        return *index;
    }

    let payloads = &mut builder.frame.lanes.mark_payloads;
    let index = payloads.len() as u32;
    payloads.push(FrameMarkPayload {
        payload: payload.clone(),
        cells: cells.cells.clone(),
    });
    builder.mark_payloads.insert(key, index);
    index
}

/// Draws the placements of oversize shapes as paths after the item's other prims.
///
/// Each path fills its shape at the quantised device anchor and is cut to the item's bounds.
/// With `UNION` the placements of the item are filled as one path.
fn oversize(
    builder: &mut Builder<'_>,
    placed: &Placed,
    marks: &Marks,
    payload: &MarkPayload,
    cells: &MarkCells,
    paint: PaintRef,
) {
    if !cells.oversize.iter().any(|large| *large) {
        return;
    }

    let anchored = anchored(placed, marks, payload, cells);
    let union = marks.flags.contains(MarkFlags::UNION);
    if union && let Some((_, _, pin)) = anchored.first() {
        let pin = *pin;
        let mut joined = BezPath::new();
        for (at, shape, _) in &anchored {
            let moved = Affine::translate((f64::from(at[0]), f64::from(at[1]))) * shape.as_ref();
            joined.extend(moved.elements().iter().copied());
        }
        draw(
            builder,
            placed,
            marks,
            [0.0; 2],
            Arc::new(joined),
            paint,
            pin,
        );
        return;
    }

    for (at, shape, pin) in anchored {
        draw(builder, placed, marks, at, shape, paint, pin);
    }
}

/// Returns the drawn placements of oversize shapes, each with its device anchor quantised to
/// quarter pixels, its shape and its pin.
///
/// Placements whose anchor maps to a `w` of zero or below are left out.
fn anchored(
    placed: &Placed,
    marks: &Marks,
    payload: &MarkPayload,
    cells: &MarkCells,
) -> Vec<([f32; 2], Arc<BezPath>, Pin)> {
    let local_to_device = placed.world * translation(placed.offset);
    let drawn = payload
        .placements
        .iter()
        .skip(first_placement(marks) as usize)
        .take(marks.placements as usize);

    let mut anchored = Vec::new();
    for placement in drawn {
        if !cells.oversize[placement.shape as usize] {
            continue;
        }

        let [a, b, c, d] = marks.axes;
        let local = [
            a * placement.x + c * placement.y + marks.origin[0],
            b * placement.x + d * placement.y + marks.origin[1],
        ];
        let Some(anchor) = map_point(&local_to_device, local[0], local[1]) else {
            continue;
        };
        let quantised = anchor.map(|axis| (4.0 * axis).round() / 4.0);
        let shape = &payload.shapes[placement.shape as usize];
        anchored.push((quantised, shape.clone(), Pin { local, anchor }));
    }

    anchored
}

/// A placement's anchor in local space and on the device, which ties the item's paint to the
/// device under a projective matrix.
#[derive(Clone, Copy)]
struct Pin {
    local: [f32; 2],
    anchor: [f32; 2],
}

/// Draws `shape`, an outline in device pixels placed at `at`, cut to the item's bounds.
///
/// The paint is evaluated in the item's local space under an affine world matrix. Under a
/// projective one, local space is taken as the device moved so that the local point of `pin`
/// lands on its device point.
fn draw(
    builder: &mut Builder<'_>,
    placed: &Placed,
    marks: &Marks,
    at: [f32; 2],
    shape: Arc<BezPath>,
    paint: PaintRef,
    pin: Pin,
) {
    let place = Affine::translate((f64::from(at[0]), f64::from(at[1])));
    let local_to_device = placed.world * translation(placed.offset);
    let Some((affine, device_bounds)) = paint_and_bounds(&local_to_device, marks.bounds, pin)
    else {
        return;
    };

    // The bounds on the device, mapped into the path's space.
    let cut = place.inverse() * device_bounds;

    let device = path::to_bounds(place.transform_rect_bbox(shape.control_box()));
    let ink = placed.cut(
        device.intersect(
            Bounds::of(marks.bounds)
                .mapped(&local_to_device)
                .unwrap_or(builder.env.output),
        ),
    );
    if !item::shows(builder, ink) {
        return;
    }

    let paint_space = affine
        * Affine::translate((
            f64::from(marks.paint_origin[0]),
            f64::from(marks.paint_origin[1]),
        ));
    let frame_path = FramePath {
        path: shape,
        transform: place,
        brush: place.inverse() * paint_space,
        fill: Some(PathFill {
            paint,
            rule: FillRule::NON_ZERO,
        }),
        stroke: None,
        clips: Arc::new([PathClip {
            path: Arc::new(cut),
            rule: FillRule::NON_ZERO,
        }]),
        ink: ink.round_out().expect("a shown ink is not empty"),
    };
    path::push(builder, placed, frame_path);
}

/// Returns the map from the item's local space to the space its paint is evaluated in, and the
/// item's `bounds` as an outline on the device.
///
/// Returns `None` when a corner of the bounds maps to a `w` of zero or below.
fn paint_and_bounds(
    local_to_device: &Matrix4,
    bounds: [f32; 4],
    pin: Pin,
) -> Option<(Affine, BezPath)> {
    let [x, y, width, height] = bounds;
    if let Some(affine) = plane_affine(local_to_device) {
        let rect = Rect::new(
            f64::from(x),
            f64::from(y),
            f64::from(x + width),
            f64::from(y + height),
        );
        return Some((affine, affine * rect.to_path(0.1)));
    }

    let corners = [
        [x, y],
        [x + width, y],
        [x + width, y + height],
        [x, y + height],
    ];
    let mut outline = BezPath::new();
    for (index, [cx, cy]) in corners.into_iter().enumerate() {
        let [dx, dy] = map_point(local_to_device, cx, cy)?;
        let point = (f64::from(dx), f64::from(dy));
        if index == 0 {
            outline.move_to(point);
        } else {
            outline.line_to(point);
        }
    }
    outline.close_path();

    let moved = [0, 1].map(|axis| f64::from(pin.anchor[axis] - pin.local[axis]));
    Some((Affine::translate((moved[0], moved[1])), outline))
}

/// Returns the first placement an item draws.
fn first_placement(marks: &Marks) -> u32 {
    let counts = [marks.discs, marks.boxes, marks.vertices, marks.placements];
    if counts.iter().filter(|count| **count > 0).count() == 1 {
        marks.first
    } else {
        0
    }
}
