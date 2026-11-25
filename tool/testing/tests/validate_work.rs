//! Frame validation of damage, resource work, chunk notes and capabilities, frame by frame.

mod common;

use std::sync::Arc;

use iceblink::damage::DamageSet;
use iceblink::{
    ChunkNote, DrawnChunk, Frame, ItemSource, LoweredChunk, PageFormat, PageOp, RenderCapabilities,
    ShaderOp, Shift,
};
use iceblink_testing::validate::Rule;
use iceblink_testing::{FrameValidator, validate_frame};

use crate::common::frames::{
    SIDE, batch, create_page, first, quad, rect, rules, upload, with_quad,
};

#[test]
fn an_empty_frame_and_a_first_frame_pass() {
    assert_eq!(validate_frame(&Frame::default()), Ok(()));
    assert_eq!(FrameValidator::new().check(&with_quad()), Ok(()));
}

#[test]
fn an_incremental_frame_passes_on_its_own_and_fails_as_a_first_frame() {
    // A resident quad of a revision inserted earlier, under tables the frame does not replace.
    let mut frame = Frame {
        size: [SIDE, SIDE],
        ..Frame::default()
    };
    frame.drawn.push(DrawnChunk {
        revision: 7,
        offset: [0.0, 0.0],
    });
    frame.lanes.quads.sources.push(ItemSource::resident(0, 3));
    frame.batches.push(batch(iceblink::BatchKind::Quads, 0..1));
    upload(&mut frame, 2, rect(0, 0, 4, 4));

    assert_eq!(validate_frame(&frame), Ok(()));
    let violations = rules(FrameValidator::new().check(&frame));
    assert!(violations.contains(&Rule::Sources), "{violations:?}");
    assert!(violations.contains(&Rule::PageOps), "{violations:?}");
}

#[test]
fn damage_outside_the_output_is_reported() {
    let mut frame = first();
    let mut draw = DamageSet::new();
    draw.absorb(rect(60, 60, 8, 8));
    frame.damage.draw = draw;
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::Damage]);
}

#[test]
fn a_shift_must_expose_only_drawn_pixels() {
    let mut frame = first();
    frame.damage.draw = DamageSet::new();
    frame.shifts.push(Shift {
        region: rect(0, 0, 32, 32),
        by: [0, -4],
    });
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::Damage]);

    frame.damage.draw.absorb(rect(0, 28, 32, 4));
    assert_eq!(validate_frame(&frame), Ok(()));
}

#[test]
fn page_operations_follow_the_pages_the_renderer_holds() {
    let mut frame = first();
    create_page(&mut frame, 0, PageFormat::A8);
    upload(&mut frame, 0, rect(0, 0, 8, 8));
    assert_eq!(validate_frame(&frame), Ok(()));

    create_page(&mut frame, 0, PageFormat::A8);
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::PageOps]);

    let mut frame = first();
    frame.resources.pages.push(PageOp::Destroy { page: 1 });
    upload(&mut frame, 1, rect(0, 0, 8, 8));
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::PageOps]);
}

#[test]
fn an_upload_stays_inside_its_page_and_its_bytes() {
    let mut frame = first();
    create_page(&mut frame, 0, PageFormat::A8);
    upload(&mut frame, 0, rect(60, 0, 8, 8));
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::PageOps]);

    let mut frame = first();
    create_page(&mut frame, 0, PageFormat::RGBA8_PREMULTIPLIED);
    // One byte per texel, where the page takes four.
    upload(&mut frame, 0, rect(0, 0, 8, 8));
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::PageOps]);
}

#[test]
fn an_upload_names_a_level_of_its_page_and_stays_inside_it() {
    let levels = |levels: u32, level: u32, side: i32| {
        let mut frame = first();
        frame.resources.pages.push(PageOp::Create {
            page: 0,
            format: PageFormat::A8,
            size: [SIDE, SIDE],
            levels,
        });
        frame.resources.pages.push(PageOp::Upload {
            page: 0,
            level,
            rect: rect(0, 0, side, side),
            bytes: Arc::from(vec![0u8; (side * side) as usize]),
            offset: 0,
            stride: side as u32,
        });
        rules(validate_frame(&frame))
    };
    assert_eq!(
        levels(3, 2, 16),
        vec![],
        "level 2 of a 64-texel page is 16 texels"
    );
    assert_eq!(levels(3, 2, 32), vec![Rule::PageOps], "outside level 2");
    assert_eq!(levels(3, 3, 8), vec![Rule::PageOps], "past the last level");
    assert_eq!(
        levels(8, 0, 8),
        vec![Rule::Values],
        "a 64-texel page has 7 levels"
    );
    assert_eq!(levels(0, 0, 8), vec![Rule::Values]);
}

#[test]
fn shader_slots_are_created_once_and_destroyed_while_live() {
    let mut frame = first();
    frame
        .resources
        .shaders
        .push(ShaderOp::Destroy { shader: 3 });
    assert_eq!(validate_frame(&frame), Ok(()));
    assert_eq!(
        rules(FrameValidator::new().check(&frame)),
        vec![Rule::ShaderOps]
    );
}

#[test]
fn a_retired_revision_is_neither_drawn_nor_inserted_again() {
    let chunk = Arc::new(LoweredChunk {
        quads: vec![quad([0.0, 0.0, 8.0, 8.0])],
        ..LoweredChunk::default()
    });
    let mut frame = first();
    frame.chunk_notes.push(ChunkNote::Inserted {
        revision: 1,
        chunk: chunk.clone(),
    });
    frame.chunk_notes.push(ChunkNote::Retired { revision: 1 });
    frame.drawn.push(DrawnChunk {
        revision: 1,
        offset: [0.0, 0.0],
    });
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::Sources]);

    frame.drawn.clear();
    frame
        .chunk_notes
        .push(ChunkNote::Inserted { revision: 1, chunk });
    assert_eq!(rules(validate_frame(&frame)), vec![Rule::ChunkNotes]);
}

#[test]
fn a_validator_follows_pages_across_frames_and_a_lost_device() {
    let mut validator = FrameValidator::new();
    let mut creating = first();
    create_page(&mut creating, 0, PageFormat::A8);
    assert_eq!(validator.check(&creating), Ok(()));

    let mut uploading = first();
    upload(&mut uploading, 0, rect(0, 0, 8, 8));
    assert_eq!(validator.check(&uploading), Ok(()));

    validator.device_lost();
    assert_eq!(rules(validator.check(&uploading)), vec![Rule::PageOps]);
}

#[test]
fn capabilities_rule_out_shifts_resident_chunks_and_subpixel_sprites() {
    let mut frame = first();
    frame.shifts.push(Shift {
        region: rect(0, 0, 32, 32),
        by: [0, 0],
    });
    let mut validator = FrameValidator::new();
    validator.set_capabilities(Some(RenderCapabilities::MINIMAL));
    assert_eq!(rules(validator.check(&frame)), vec![Rule::Capabilities]);

    let mut validator = FrameValidator::new();
    validator.set_capabilities(Some(RenderCapabilities {
        can_shift: true,
        ..RenderCapabilities::MINIMAL
    }));
    assert_eq!(validator.check(&frame), Ok(()));
}
