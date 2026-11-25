//! The frame's container behaviour: reuse, table deltas, resident moves and plan lookups.

mod common;

use std::sync::Arc;

use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    AtlasTile, Batch, BatchKind, ColorSprite, Frame, FrameMarks, FrameShadow, Instance, ItemSource,
    Line, LoweredChunk, MarkBin, MarkPlan, Matrix4, PaintRef, ROUND, Shadow, Sprite, SpriteFlags,
    TableDelta,
};
use iceblink::{ClipRef, Color, MarkFlags, Marks, SpaceRef};

#[test]
fn a_frame_and_its_resident_chunks_can_move_between_threads() {
    fn send<T: Send + Sync>() {}

    send::<Frame>();
    send::<Arc<LoweredChunk>>();
}

#[test]
fn clearing_a_frame_keeps_its_allocations() {
    let mut frame = Frame::default();
    frame.lanes.quads.items.push(common::quad(0.0));
    frame.lanes.quads.sources.push(ItemSource::transient(0));
    frame.batches.push(Batch {
        kind: BatchKind::Quads,
        range: 0..1,
        ink: Rect::new(Point::new(0, 0), Size::new(10, 10)),
    });
    frame.tables.spaces.replace = true;
    frame.tables.spaces.all.push(Matrix4::IDENTITY);
    let capacity = frame.lanes.quads.items.capacity();
    let table_capacity = frame.tables.spaces.all.capacity();

    frame.clear();

    assert!(frame.lanes.quads.items.is_empty());
    assert!(frame.lanes.quads.sources.is_empty());
    assert!(frame.batches.is_empty());
    assert!(frame.tables.spaces.is_unchanged());
    assert_eq!(frame.lanes.quads.items.capacity(), capacity);
    assert_eq!(frame.tables.spaces.all.capacity(), table_capacity);
}

#[test]
fn a_changed_slot_replaces_its_value_after_a_replacement_and_one_past_the_end_grows_the_table() {
    let mut table = vec![1, 2, 3];

    let mut delta = TableDelta::default();
    delta.changed.extend([(1, 20), (5, 60)]);
    delta.apply(&mut table);
    assert_eq!(table.len(), 6);
    assert_eq!([table[0], table[1], table[2], table[5]], [1, 20, 3, 60]);

    delta.clear();
    delta.apply(&mut table);
    assert_eq!(table.len(), 6);

    delta.replace = true;
    delta.all.push(7);
    delta.changed.push((1, 8));
    delta.apply(&mut table);
    assert_eq!(table, [7, 8]);
}

#[test]
fn moving_an_instance_moves_its_rectangle_origins_and_paint_origin_only() {
    let by = [3.0, -2.0];

    let quad = common::quad(1.0).moved(by);
    assert_eq!(quad.bounds, [4.0, -2.0, 10.0, 10.0]);
    assert_eq!(quad.paint_origin, by);

    let colored = color_sprite().moved(by);
    assert_eq!(colored.bounds, [3.0, -2.0, 4.0, 4.0]);
    assert_eq!(colored.frame, [4.0, -1.0, 2.0, 2.0]);
    assert_eq!(colored.paint_origin, by);
    assert_eq!(colored.tile, color_sprite().tile);

    let shadow = frame_shadow().moved(by);
    assert_eq!(shadow.bounds, [3.0, -2.0, 30.0, 30.0]);
    assert_eq!(shadow.shadow.shape_bounds, [8.0, 3.0, 20.0, 20.0]);
    assert_eq!(shadow.shadow.caster_bounds, [8.0, 3.0, 20.0, 20.0]);

    let marks = frame_marks().moved(by);
    assert_eq!(marks.marks.bounds, [3.0, -2.0, 8.0, 8.0]);
    assert_eq!(marks.marks.origin, [4.0, -1.0]);
    assert_eq!(marks.marks.paint_origin, by);
    assert_eq!(marks.marks.axes, frame_marks().marks.axes);

    let line = line().moved(by);
    assert_eq!(line.bounds, [3.0, -2.0, 5.0, 1.0]);

    let mono = sprite().moved(by);
    assert_eq!(mono.bounds, [3.0, -2.0, 4.0, 4.0]);
    assert_eq!(mono.paint_origin, by);
}

#[test]
fn mark_bins_are_found_by_lane_position() {
    let bin = |item| MarkBin {
        item,
        region: Rect::new(Point::new(0, 0), Size::new(4, 4)),
        at: [0, 0],
        page: 0,
    };
    let plan = MarkPlan {
        bins: vec![bin(2), bin(5), bin(9)],
        overflow: vec![bin(7)],
        pages: 2,
        extent: [64, 64],
        full_damage: false,
    };

    assert_eq!(plan.bin(5).map(|found| found.item), Some(5));
    assert_eq!(plan.bin(7), None);
    assert_eq!(plan.overflow_bin(7).map(|found| found.item), Some(7));
}

fn tile() -> AtlasTile {
    AtlasTile {
        page: 1,
        rect: [2, 3, 4, 4],
    }
}

fn sprite() -> Sprite {
    Sprite {
        bounds: [0.0, 0.0, 4.0, 4.0],
        paint: PaintRef::NONE,
        paint_origin: [0.0, 0.0],
        tile: tile(),
        clip: 0,
        space: 0,
        _pad: 0,
    }
}

fn color_sprite() -> ColorSprite {
    ColorSprite {
        bounds: [0.0, 0.0, 4.0, 4.0],
        frame: [1.0, 1.0, 2.0, 2.0],
        radii: [0.0; 8],
        shapes: ROUND,
        tile: tile(),
        paint: PaintRef::NONE,
        paint_origin: [0.0, 0.0],
        opacity: 1.0,
        clip: 0,
        space: 0,
        flags: SpriteFlags::default(),
    }
}

fn frame_shadow() -> FrameShadow {
    FrameShadow {
        bounds: [0.0, 0.0, 30.0, 30.0],
        shadow: Shadow {
            shape_bounds: [5.0, 5.0, 20.0, 20.0],
            radii: [0.0; 8],
            caster_bounds: [5.0, 5.0, 20.0, 20.0],
            caster_radii: [0.0; 8],
            shapes: ROUND,
            color: Color::TRANSPARENT,
            blur: 2.0,
            clip: ClipRef::NONE,
            space: SpaceRef::INHERIT,
            inset: 0,
        },
    }
}

fn frame_marks() -> FrameMarks {
    FrameMarks {
        marks: Marks {
            bounds: [0.0, 0.0, 8.0, 8.0],
            paint: PaintRef::NONE,
            paint_origin: [0.0, 0.0],
            origin: [1.0, 1.0],
            axes: [1.0, 0.0, 0.0, 1.0],
            half_width: 0.5,
            discs: 1,
            boxes: 0,
            vertices: 0,
            placements: 0,
            first: 0,
            payload: 0,
            clip: ClipRef::NONE,
            space: SpaceRef::INHERIT,
            flags: MarkFlags::default(),
        },
        page: 0,
    }
}

fn line() -> Line {
    Line {
        bounds: [0.0, 0.0, 5.0, 1.0],
        color: Color::TRANSPARENT,
        thickness: 1.0,
        clip: ClipRef::NONE,
        space: SpaceRef::INHERIT,
        style: Default::default(),
        _pad: [0; 3],
    }
}
