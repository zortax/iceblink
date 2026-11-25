//! Frame validation of lane sources and the slots, values, tiles and payloads of instances.

mod common;

use std::sync::Arc;

use iceblink::{
    AtlasTile, BatchKind, BorderStyle, Color, FrameMarkPayload, FramePaint, ItemSource, MarkFlags,
    MarkPayload, Matrix4, PageFormat, PaintKind, PaintRef,
};
use iceblink_testing::validate::Rule;
use iceblink_testing::validate_frame;

use crate::common::frames::{
    batch, create_page, first, marks, push_quad, quad, rules, solid, sprite,
};

#[test]
fn a_transient_source_names_an_item_of_its_lane() {
    let mut frame = first();
    push_quad(&mut frame, quad([0.0, 0.0, 8.0, 8.0]));
    frame.lanes.quads.sources[0] = ItemSource::transient(4);
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::Sources]);
}

#[test]
fn a_run_draws_at_least_one_instance_inside_its_lane() {
    let mut frame = first();
    push_quad(&mut frame, quad([0.0, 0.0, 8.0, 8.0]));
    push_quad(&mut frame, quad([10.0, 0.0, 8.0, 8.0]));
    frame.lanes.quads.sources = vec![ItemSource {
        chunk: ItemSource::TRANSIENT,
        index: 0,
        len: 2,
    }];
    assert_eq!(rules(validate_frame(&frame)), vec![]);

    // The third instance names no item, and no batch draws it.
    frame.lanes.quads.sources[0].len = 3;
    assert_eq!(
        rules(validate_frame(&frame)),
        vec![Rule::Sources, Rule::BatchCoverage]
    );
}

#[test]
fn a_paint_names_a_slot_of_the_replaced_table() {
    let mut frame = first();
    let mut outside = quad([0.0, 0.0, 8.0, 8.0]);
    outside.fill = solid(5);
    push_quad(&mut frame, outside);
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::Slots]);
}

#[test]
fn a_paint_reference_has_the_kind_of_its_paint() {
    let mut frame = first();
    let mut wrong = quad([0.0, 0.0, 8.0, 8.0]);
    wrong.fill = PaintRef {
        kind: PaintKind::GRADIENT,
        ..solid(0)
    };
    push_quad(&mut frame, wrong);
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::PaintKinds]);
}

#[test]
fn enumerations_and_colours_hold_listed_values() {
    let mut frame = first();
    let mut styled = quad([0.0, 0.0, 8.0, 8.0]);
    styled.style = BorderStyle(9);
    push_quad(&mut frame, styled);
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::Values]);

    let mut frame = first();
    frame.tables.paints.all = vec![FramePaint::Solid(Color([1.0, 0.0, 0.0, 0.5]))];
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::Values]);
}

/// Adds a mono sprite of a tile on page 0 at `origin` and its batch.
fn push_sprite(frame: &mut iceblink::Frame, origin: [f32; 2]) {
    let tile = AtlasTile {
        page: 0,
        rect: [0, 0, 8, 12],
    };
    frame.lanes.mono.items.push(sprite(origin, tile));
    frame.lanes.mono.sources.push(ItemSource::transient(0));
    frame.batches.push(batch(BatchKind::Mono { page: 0 }, 0..1));
}

#[test]
fn a_mono_sprite_reads_a_coverage_page() {
    let mut frame = first();
    create_page(&mut frame, 0, PageFormat::A8);
    push_sprite(&mut frame, [4.0, 4.0]);
    assert_eq!(validate_frame(&frame), Ok(()));

    let mut frame = first();
    create_page(&mut frame, 0, PageFormat::RGBA8_PREMULTIPLIED);
    push_sprite(&mut frame, [4.0, 4.0]);
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::Tiles]);
}

#[test]
fn a_glyph_under_a_translation_lands_on_whole_pixels() {
    let mut frame = first();
    frame.tables.spaces.all = vec![Matrix4::translation(0.5, 0.0, 0.0)];
    create_page(&mut frame, 0, PageFormat::A8);
    push_sprite(&mut frame, [4.0, 4.0]);
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::WholePixels]);

    frame.lanes.mono.items[0].bounds[0] = 3.5;
    assert_eq!(validate_frame(&frame), Ok(()));
}

#[test]
fn marks_read_inside_their_payload() {
    let mut frame = first();
    frame.lanes.marks.items.push(marks(3, MarkFlags::default()));
    frame.lanes.marks.sources.push(ItemSource::transient(0));
    frame.lanes.mark_payloads.push(FrameMarkPayload {
        payload: Arc::new(MarkPayload {
            discs: vec![[4.0, 4.0, 2.0, 0.0]; 2],
            ..MarkPayload::default()
        }),
        cells: Arc::from(Vec::new()),
    });
    frame.batches.push(batch(BatchKind::Marks, 0..1));
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::MarkPayloads]);

    frame.lanes.marks.items[0].marks.discs = 2;
    assert_eq!(validate_frame(&frame), Ok(()));
}
