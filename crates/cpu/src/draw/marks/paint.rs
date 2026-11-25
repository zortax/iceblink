//! Painting a mark item's prims one by one, and painting a union item through its bin.

use iceblink::geom::{Device, Rect};
use iceblink::{FrameMarkPayload, FrameMarks, Marks};

use crate::draw::footprint::{footprint, holds};
use crate::draw::marks::bins::Bin;
use crate::draw::marks::glyph;
use crate::draw::marks::mapping::{Mapping, Sample, margin_of};
use crate::draw::marks::prims::{
    SegmentQuad, box_coverage, box_quad, corners, disc_coverage, disc_quad, inside, is_separator,
    segment_caps, segment_value,
};
use crate::draw::{Scene, paint_area};
use crate::math::edge::sdf_coverage;
use crate::math::vec::scale4;
use crate::tables::{Clip, Painter};
use crate::target::Surface;

/// Paints every prim of `item` on its own: discs, boxes, polylines, then placements.
pub(super) fn prims(
    scene: &Scene<'_>,
    surface: &mut Surface<'_>,
    within: Rect<i32, Device>,
    item: &FrameMarks,
    payload: &FrameMarkPayload,
    row: &mut Vec<[f32; 4]>,
) {
    let marks = &item.marks;
    let mapping = Mapping::of(marks, scene.space(marks.space.0));
    let clip = scene.clip(marks.clip.0);
    let paint = Painter::of(scene.paints, marks.paint);
    let data = &payload.payload;
    let first = marks.first as usize;

    let painted = |surface: &mut Surface<'_>,
                   row: &mut Vec<[f32; 4]>,
                   corners: [[f32; 2]; 4],
                   holds: &dyn Fn([f32; 2]) -> bool,
                   coverage: &dyn Fn(&Sample) -> f32| {
        let Some(area) = mapping.footprint(corners, within) else {
            return;
        };
        paint_area(surface, area, row, |device| {
            let Some(sample) = mapping.sample(device) else {
                return [0.0; 4];
            };
            if !holds(sample.point) {
                return [0.0; 4];
            }
            let covered = coverage(&sample);
            colour(
                scene,
                clip,
                &paint,
                marks.paint_origin,
                device,
                sample.local,
                covered,
            )
        });
    };

    for disc in data.discs.iter().skip(first).take(marks.discs as usize) {
        let (low, high) = disc_quad(&mapping, *disc);
        painted(
            surface,
            row,
            corners(low, high),
            &|point| inside(point, low, high),
            &|sample| disc_coverage(&mapping, marks.flags, *disc, sample),
        );
    }
    for prim in data.boxes.iter().skip(first).take(marks.boxes as usize) {
        let (low, high) = box_quad(&mapping, prim);
        painted(
            surface,
            row,
            corners(low, high),
            &|point| inside(point, low, high),
            &|sample| box_coverage(&mapping, prim, sample),
        );
    }
    if marks.vertices >= 3 {
        let vertices = &data.vertices;
        for at in first + 1..first + marks.vertices as usize - 2 {
            let (Some(&raw_a), Some(&raw_b)) = (vertices.get(at), vertices.get(at + 1)) else {
                continue;
            };
            if is_separator(raw_a) || is_separator(raw_b) {
                continue;
            }
            let a = mapping.payload_point(raw_a);
            let b = mapping.payload_point(raw_b);
            let quad = SegmentQuad::new(a, b, marks.half_width, mapping.margin);
            let segment = Segment {
                at,
                a,
                b,
                caps: segment_caps(vertices, at, marks.flags),
            };
            painted(
                surface,
                row,
                quad.corners(),
                &|point| quad.holds(point),
                &|sample| owned_coverage(&mapping, marks, vertices, &segment, sample),
            );
        }
    }
    glyph::paint(scene, surface, within, item, payload, row);
}

// A polyline segment from vertex `at` to vertex `at + 1`, in distance space.
struct Segment {
    at: usize,
    a: [f32; 2],
    b: [f32; 2],
    caps: u32,
}

// The coverage of `segment` at `sample`. A pixel near a join is painted by the nearer segment only.
fn owned_coverage(
    mapping: &Mapping<'_>,
    marks: &Marks,
    vertices: &[[f32; 2]],
    segment: &Segment,
    sample: &Sample,
) -> f32 {
    let Segment { at, a, b, caps } = *segment;
    let half = marks.half_width;
    let own = segment_value(sample.point, a, b, half, caps);
    let mut owned = true;
    if let Some(&before) = at.checked_sub(1).and_then(|index| vertices.get(index))
        && !is_separator(before)
    {
        let other_caps = segment_caps(vertices, at - 1, marks.flags);
        let other = segment_value(
            sample.point,
            mapping.payload_point(before),
            a,
            half,
            other_caps,
        );
        owned &= own.d < other.d;
    }
    if let Some(&after) = vertices.get(at + 2)
        && !is_separator(after)
    {
        let other_caps = segment_caps(vertices, at + 1, marks.flags);
        let other = segment_value(
            sample.point,
            b,
            mapping.payload_point(after),
            half,
            other_caps,
        );
        owned &= own.d <= other.d;
    }
    if owned {
        sdf_coverage(own, sample.across, sample.down)
    } else {
        0.0
    }
}

/// Paints a union item through its bin: the item's bounds, grown by a device pixel, with the
/// coverage the bin holds at each pixel.
pub(super) fn composite(
    scene: &Scene<'_>,
    surface: &mut Surface<'_>,
    within: Rect<i32, Device>,
    item: &FrameMarks,
    bin: &Bin,
    row: &mut Vec<[f32; 4]>,
) {
    let marks = &item.marks;
    let space = scene.space(marks.space.0);
    let columns = space.matrix().columns;
    let margin = margin_of(
        [columns[0][0], columns[0][1]],
        [columns[1][0], columns[1][1]],
    );
    let grown = [
        marks.bounds[0] - margin,
        marks.bounds[1] - margin,
        marks.bounds[2] + 2.0 * margin,
        marks.bounds[3] + 2.0 * margin,
    ];
    let Some(area) = footprint(space, grown, within) else {
        return;
    };
    let clip = scene.clip(marks.clip.0);
    let paint = Painter::of(scene.paints, marks.paint);
    paint_area(surface, area, row, |device| {
        let Some(local) = space.local(device) else {
            return [0.0; 4];
        };
        if !holds(grown, local) {
            return [0.0; 4];
        }
        let covered = bin.coverage(device);
        colour(
            scene,
            clip,
            &paint,
            marks.paint_origin,
            device,
            local,
            covered.min(1.0),
        )
    });
}

/// Returns the premultiplied colour a prim paints at `device` for `coverage`.
pub(super) fn colour(
    scene: &Scene<'_>,
    clip: Option<&Clip>,
    paint: &Painter<'_>,
    paint_origin: [f32; 2],
    device: [f32; 2],
    local: [f32; 2],
    coverage: f32,
) -> [f32; 4] {
    if coverage <= 0.0 {
        return [0.0; 4];
    }
    let clipped = scene.clip_coverage(clip, device);
    if clipped <= 0.0 {
        return [0.0; 4];
    }
    scale4(paint.at(local, paint_origin), coverage * clipped)
}
