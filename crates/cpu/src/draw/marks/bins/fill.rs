//! Filling a union item's bin from its prims.
//!
//! Polylines are written first and keep the largest coverage of each quarter. Discs, boxes and
//! placements then add their coverage of the whole pixel to each quarter, which saturates at one.

use iceblink::geom::{Device, Rect};
use iceblink::{FrameMarkPayload, FrameMarks, MarkBin, Marks};

use crate::draw::Scene;
use crate::draw::marks::bins::Bin;
use crate::draw::marks::glyph;
use crate::draw::marks::mapping::{Mapping, Sample};
use crate::draw::marks::prims::{
    SegmentQuad, box_coverage, box_quad, corners, disc_coverage, disc_quad, inside, is_separator,
    segment_caps, segment_value,
};
use crate::math::edge::sdf_coverage;
use crate::pages::Page;

/// Returns the bin `planned` filled with the prims of `item`.
pub(super) fn filled(
    scene: &Scene<'_>,
    planned: &MarkBin,
    item: &FrameMarks,
    payload: &FrameMarkPayload,
) -> Bin {
    let region = planned.region;
    let mut bin = Bin {
        region,
        quarters: vec![[0; 4]; region.width().max(0) as usize * region.height().max(0) as usize],
    };
    if region.is_empty() {
        return bin;
    }

    let marks = &item.marks;
    let mapping = Mapping::of(marks, scene.space(marks.space.0));
    keep_polylines(&mut bin, &mapping, marks, payload);
    add_discs(&mut bin, &mapping, marks, payload);
    add_boxes(&mut bin, &mapping, marks, payload);
    if marks.placements > 0
        && let Some(page) = scene.pages.get(item.page)
    {
        add_placements(&mut bin, &mapping, item, payload, page);
    }
    bin
}

// Keeps the largest coverage of each quarter by the polyline segments.
fn keep_polylines(bin: &mut Bin, mapping: &Mapping<'_>, marks: &Marks, payload: &FrameMarkPayload) {
    if marks.vertices < 3 {
        return;
    }
    let region = bin.region;
    let vertices = &payload.payload.vertices;
    let first = marks.first as usize;
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
        let caps = segment_caps(vertices, at, marks.flags);
        each_pixel(
            mapping,
            region,
            quad.corners(),
            |point| quad.holds(point),
            |x, y, sample| {
                // Each quarter is half a pixel wide, and measured from its own centre.
                let half_across = sample.across.map(|value| value * 0.5);
                let half_down = sample.down.map(|value| value * 0.5);
                let quarters = [0u32, 1, 2, 3].map(|quarter| {
                    let corner = [(quarter & 1) as f32 - 0.5, (quarter >> 1) as f32 - 0.5];
                    let centre = [
                        sample.point[0] + corner[0] * half_across[0] + corner[1] * half_down[0],
                        sample.point[1] + corner[0] * half_across[1] + corner[1] * half_down[1],
                    ];
                    let distance = segment_value(centre, a, b, marks.half_width, caps);
                    sdf_coverage(distance, half_across, half_down)
                });
                bin.keep_largest(x, y, quarters);
            },
        );
    }
}

// Adds the coverage of the discs.
fn add_discs(bin: &mut Bin, mapping: &Mapping<'_>, marks: &Marks, payload: &FrameMarkPayload) {
    let region = bin.region;
    let discs = payload.payload.discs.iter().skip(marks.first as usize);
    for disc in discs.take(marks.discs as usize) {
        let (low, high) = disc_quad(mapping, *disc);
        each_pixel(
            mapping,
            region,
            corners(low, high),
            |point| inside(point, low, high),
            |x, y, sample| {
                bin.add(x, y, disc_coverage(mapping, marks.flags, *disc, sample));
            },
        );
    }
}

// Adds the coverage of the boxes.
fn add_boxes(bin: &mut Bin, mapping: &Mapping<'_>, marks: &Marks, payload: &FrameMarkPayload) {
    let region = bin.region;
    let boxes = payload.payload.boxes.iter().skip(marks.first as usize);
    for prim in boxes.take(marks.boxes as usize) {
        let (low, high) = box_quad(mapping, prim);
        each_pixel(
            mapping,
            region,
            corners(low, high),
            |point| inside(point, low, high),
            |x, y, sample| {
                bin.add(x, y, box_coverage(mapping, prim, sample));
            },
        );
    }
}

// Adds the coverage of the placements' cells on `page`.
fn add_placements(
    bin: &mut Bin,
    mapping: &Mapping<'_>,
    item: &FrameMarks,
    payload: &FrameMarkPayload,
    page: &Page,
) {
    let region = bin.region;
    for cell in glyph::placed(item, payload, mapping) {
        let Some(area) = cell.pixels.intersection(region) else {
            continue;
        };
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                bin.add(x, y, cell.coverage(page, x, y));
            }
        }
    }
}

// Calls `visit` with each pixel of `region` within the distance-space quadrilateral `corners`
// whose sample `holds` accepts.
fn each_pixel(
    mapping: &Mapping<'_>,
    region: Rect<i32, Device>,
    corners: [[f32; 2]; 4],
    holds: impl Fn([f32; 2]) -> bool,
    mut visit: impl FnMut(i32, i32, &Sample),
) {
    let Some(area) = mapping.footprint(corners, region) else {
        return;
    };
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let Some(sample) = mapping.sample([x as f32 + 0.5, y as f32 + 0.5]) else {
                continue;
            };
            if holds(sample.point) {
                visit(x, y, &sample);
            }
        }
    }
}
