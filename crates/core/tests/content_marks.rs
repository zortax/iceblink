//! Marks items and the phase cells of their shapes.

mod common;

use std::sync::Arc;

use common::content::{drain, solid, upload_bytes};
use iceblink::kurbo::{BezPath, Rect, Shape};
use iceblink::{
    ClipRef, MarkFlags, MarkPayload, MarkPlacement, Marks, Matrix4, PageFormat, PageOp, SpaceRef,
};
use iceblink_core::content::{ContentCache, ContentConfig, Target};

/// Returns a square of `side` device pixels with its corner at the anchor.
fn square(side: f64) -> Arc<BezPath> {
    Arc::new(Rect::new(0.0, 0.0, side, side).to_path(0.1))
}

/// Returns a payload placing each of `shapes` once.
fn payload(shapes: Vec<Arc<BezPath>>) -> Arc<MarkPayload> {
    let placements = (0..shapes.len() as u32)
        .map(|shape| MarkPlacement {
            x: shape as f32 * 10.0,
            y: 0.0,
            shape,
            _pad: 0,
        })
        .collect();
    Arc::new(MarkPayload {
        shapes,
        placements,
        ..MarkPayload::default()
    })
}

/// Returns a marks item that draws the first `placements` placements of payload slot 3.
fn item(placements: u32) -> Marks {
    Marks {
        bounds: [0.0, 0.0, 100.0, 20.0],
        paint: solid(2),
        paint_origin: [1.0, 1.0],
        origin: [5.0, 6.0],
        axes: [1.0, 0.0, 0.0, 1.0],
        half_width: 0.0,
        discs: 0,
        boxes: 0,
        vertices: 0,
        placements,
        first: 0,
        payload: 3,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: MarkFlags::default(),
    }
}

/// Returns a target with frame slots and a chunk offset.
fn target() -> Target {
    let mut target = Target::new(Matrix4::IDENTITY);
    target.offset = [2.0, 3.0];
    target.clip = 4;
    target.space = 5;
    target
}

#[test]
fn each_shape_gets_sixteen_phase_cells_on_the_marks_page() {
    let mut cache = ContentCache::new(ContentConfig::default());
    let payload = payload(vec![square(4.0), square(2.0)]);

    let lowered = cache.lower_marks(&item(2), &payload, &target()).unwrap();

    let cells = &lowered.payload.cells;
    assert_eq!(cells.len(), 32);
    assert_eq!(cells[0].offset, [0, 0]);
    assert_eq!(&cells[0].rect[2..], &[4, 4]);
    assert_eq!(
        &cells[1].rect[2..],
        &[5, 4],
        "a quarter pixel right reaches one more column"
    );
    assert_eq!(&cells[4].rect[2..], &[4, 5], "phase_y 1 is index 4");
    assert_eq!(&cells[5].rect[2..], &[5, 5]);
    let ops = drain(&mut cache);
    let (page, size) = match ops[0] {
        PageOp::Create {
            page, format, size, ..
        } => {
            assert_eq!(format, PageFormat::A8);
            (page, size)
        }
        ref other => panic!("expected a page creation, got {other:?}"),
    };
    assert_eq!(lowered.marks.page, page);
    for cell in cells.iter() {
        assert!(cell.rect[0] + cell.rect[2] <= size[0]);
        assert!(cell.rect[1] + cell.rect[3] <= size[1]);
    }
    let distinct: std::collections::HashSet<_> = cells
        .iter()
        .map(|cell| (cell.rect[0], cell.rect[1]))
        .collect();
    assert_eq!(distinct.len(), 32, "cells do not overlap");
}

#[test]
fn the_phase_zero_cell_of_a_square_is_fully_covered() {
    let mut cache = ContentCache::new(ContentConfig::default());
    let payload = payload(vec![square(2.0)]);
    let lowered = cache.lower_marks(&item(1), &payload, &target()).unwrap();

    let ops = drain(&mut cache);
    let (rect, bytes, stride) = ops
        .iter()
        .find_map(|op| match op {
            PageOp::Upload { rect, stride, .. } => {
                Some((*rect, upload_bytes(op).unwrap(), *stride))
            }
            _ => None,
        })
        .unwrap();
    let cell = lowered.payload.cells[0];
    let x = (cell.rect[0] as i32 - rect.origin.x) as usize;
    let y = (cell.rect[1] as i32 - rect.origin.y) as usize;
    let at = |dx: usize, dy: usize| bytes[(y + dy) * stride as usize + x + dx];
    assert_eq!([at(0, 0), at(1, 0), at(0, 1), at(1, 1)], [255; 4]);
}

#[test]
fn payloads_with_equal_shapes_share_one_sheet() {
    let mut cache = ContentCache::new(ContentConfig::default());
    let first = payload(vec![square(3.0)]);
    let second = payload(vec![square(3.0)]);

    let a = cache.lower_marks(&item(1), &first, &target()).unwrap();
    let b = cache.lower_marks(&item(1), &second, &target()).unwrap();
    let again = cache.lower_marks(&item(1), &first, &target()).unwrap();

    assert_eq!(a.tile, b.tile);
    assert_eq!(a.tile, again.tile);
    assert!(Arc::ptr_eq(&a.payload.cells, &b.payload.cells));
    assert_eq!(cache.stats().mark_sheets_rasterised, 1);
}

#[test]
fn the_item_takes_the_frame_slots_and_the_chunk_offset() {
    let mut cache = ContentCache::new(ContentConfig::default());
    let payload = payload(vec![square(3.0)]);

    let lowered = cache.lower_marks(&item(1), &payload, &target()).unwrap();

    let marks = lowered.marks.marks;
    assert_eq!(marks.bounds, [2.0, 3.0, 100.0, 20.0]);
    assert_eq!(marks.origin, [7.0, 9.0]);
    assert_eq!(marks.paint_origin, [3.0, 4.0]);
    assert_eq!(marks.clip, ClipRef(4));
    assert_eq!(marks.space, SpaceRef(5));
    assert_eq!(marks.payload, 3);
    assert!(Arc::ptr_eq(&lowered.payload.payload, &payload));
}

#[test]
fn a_payload_without_shapes_needs_no_sheet() {
    let mut cache = ContentCache::new(ContentConfig::default());
    let empty = Arc::new(MarkPayload::default());
    let lowered = cache.lower_marks(&item(0), &empty, &target()).unwrap();
    assert!(lowered.tile.is_none());
    assert!(lowered.payload.cells.is_empty());
}

#[test]
fn a_huge_shape_is_drawn_as_a_path_and_its_neighbours_from_cells() {
    let mut cache = ContentCache::new(ContentConfig::default());
    let shapes = payload(vec![square(4.0), square(400.0)]);
    let lowered = cache.lower_marks(&item(2), &shapes, &target()).unwrap();
    let cells = &lowered.payload.cells;
    assert_eq!(cells.len(), 32);
    assert!(cells[..16].iter().all(|cell| cell.rect[2] > 0));
    assert!(cells[16..].iter().all(|cell| cell.rect[2..] == [0, 0]));

    let [path] = lowered.paths.as_slice() else {
        panic!("one placement is drawn as a path");
    };
    // The anchor is the second placement's point, moved by the origin and the offset.
    assert_eq!(path.transform.translation(), (17.0, 9.0).into());
    assert_eq!(path.ink.origin.x, 17);
    assert_eq!(path.ink.size.height, 14);
}

#[test]
fn a_huge_shape_is_drawn_as_a_path_under_a_projective_matrix() {
    let mut cache = ContentCache::new(ContentConfig::default());
    let shapes = payload(vec![square(4.0), square(400.0)]);
    // A homogeneous `w` of `1 + x / 1000`, which tilts the plane away towards the right.
    let mut projective = Matrix4::IDENTITY;
    projective.columns[0][3] = 0.001;
    let mut tilted = Target::new(projective);
    tilted.space = 5;

    let lowered = cache.lower_marks(&item(2), &shapes, &tilted).unwrap();

    let [path] = lowered.paths.as_slice() else {
        panic!("one placement is drawn as a path, got {:?}", lowered.paths);
    };
    // The second placement's point moved by the origin is (15, 6), and w is 1.015 there.
    assert_eq!(path.transform.translation(), (14.75, 6.0).into());
    assert!(path.ink.size.width > 0 && path.ink.size.height > 0);
}
