//! Glyphs rasterised at commit, atlas pages sent once and replayed to new renderers, image levels,
//! and the atlas budget, trim and forget.

mod common;

use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    BufferAge, Built, ChunkKey, ChunkNote, ClipRef, Color, Extend, Frame, FrameClock, FrameOutcome,
    FrameRequest, GlyphFlags, Image, ImageDesc, ImageFilter, ImageFlags, ImageFormat, ImageWrite,
    OrderEntry, OrderNode, PageOp, Paint, PaintKind, PaintRef, ROUND, RenderCapabilities,
    RenderTarget, SpaceRef, SubpixelOrder,
};
use iceblink::{Compositor, CompositorBudgets, OutputConfig, Renderer};
use iceblink_testing::FrameValidator;
use iceblink_testing::capture::CaptureRenderer;
use iceblink_testing::validate::describe;

use common::slice::{SIDE, Slice};
use common::story::World;
use common::text::{run, run_with};
use common::{FULL, props, stat};

type Captured = Slice<CaptureRenderer>;

/// Returns a slice past its first frame.
fn slice() -> Captured {
    let mut slice = Slice::new(CaptureRenderer::with_capabilities(FULL));
    slice.draw();
    slice
}

/// Commits chunk `owner` with one line of `text` on row `row`, and appends it to the root if
/// `append`.
fn line(slice: &mut Captured, owner: u64, row: u32, text: &str, append: bool) {
    let mut tx = slice.tx();
    let world: &mut World = &mut slice.world;
    let paint = match world.paints.first() {
        Some(&paint) => paint,
        None => world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]),
    };
    let key = ChunkKey::new(owner, 0);
    let font = world.font;
    let mut chunk = tx.record(key, props(world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    run(
        &mut chunk,
        font,
        text,
        12.0,
        [4.0, 12.0 + row as f32 * 14.0],
        fill,
    );
    if append {
        world.append(&mut tx, OrderEntry::Chunk(key));
    }
    slice.compositor.commit(tx).expect("the line commits");
}

fn uploads(slice: &Captured) -> usize {
    slice
        .frame
        .resources
        .pages
        .iter()
        .filter(|op| matches!(op, PageOp::Upload { .. }))
        .count()
}

#[test]
fn glyphs_are_rasterised_at_commit_and_not_again_when_drawn() {
    let mut slice = slice();
    line(&mut slice, 1, 0, "Rasterised once", true);
    assert!(slice.compositor.counters().commit.glyphs_rasterised > 0);

    let before = stat(&slice.compositor, "content.glyphs_rasterised");
    assert!(slice.draw());
    assert_eq!(stat(&slice.compositor, "content.glyphs_rasterised"), before);
    assert!(uploads(&slice) > 0, "the frame uploads the new glyphs");
}

#[test]
fn many_new_glyphs_are_rasterised_in_parallel_batches() {
    let mut slice = slice();
    let text: String = (0x21_u8..0x7f).map(char::from).collect();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let paint = world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
    let font = world.font;
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    for size in [10.0, 11.0, 12.0] {
        run(&mut chunk, font, &text, size, [0.0, size * 4.0], fill);
    }
    world.append(&mut tx, OrderEntry::Chunk(ChunkKey::new(1, 0)));
    slice.compositor.commit(tx).expect("the lines commit");
    assert!(stat(&slice.compositor, "content.prefetch_batches") > 0);
    assert!(slice.compositor.counters().commit.glyphs_rasterised >= 3 * 90);
}

#[test]
fn an_idle_frame_with_text_does_no_work() {
    let mut slice = slice();
    line(&mut slice, 1, 0, "Idle", true);
    assert!(slice.draw());
    let before = stat(&slice.compositor, "content.glyphs_placed");
    assert!(!slice.draw(), "nothing changed, so no frame is built");
    assert_eq!(stat(&slice.compositor, "content.glyphs_placed"), before);
}

#[test]
fn one_changed_line_costs_one_chunk_of_walk_and_upload() {
    let mut slice = slice();
    for row in 0..8 {
        line(&mut slice, 1 + row, row as u32, "the same words", true);
    }
    assert!(slice.draw());

    line(&mut slice, 4, 3, "the same wordz", false);
    assert!(slice.draw());
    let frame = slice.compositor.counters().frame;
    assert_eq!(frame.chunks_drawn, 1);
    let inserted = slice
        .frame
        .chunk_notes
        .iter()
        .filter(|note| matches!(note, ChunkNote::Inserted { .. }))
        .count();
    assert_eq!(inserted, 1, "only the changed line is lowered again");
    assert!(uploads(&slice) <= 1, "only the new glyph is uploaded");
}

#[test]
fn a_whole_pixel_move_draws_text_again_without_lowering() {
    let mut slice = slice();
    line(&mut slice, 1, 0, "Moved", true);
    assert!(slice.draw());

    let mut tx = slice.tx();
    tx.set_offset(ChunkKey::new(1, 0), [3.0, 2.0]);
    slice.compositor.commit(tx).expect("the move commits");
    assert!(slice.draw());
    assert!(
        slice.frame.chunk_notes.is_empty(),
        "the resident chunk is drawn moved"
    );
    assert!(slice.frame.resources.pages.is_empty());

    let mut tx = slice.tx();
    tx.set_offset(ChunkKey::new(1, 0), [3.5, 2.0]);
    slice.compositor.commit(tx).expect("the move commits");
    assert!(slice.draw());
    assert!(
        slice
            .frame
            .chunk_notes
            .iter()
            .any(|note| matches!(note, ChunkNote::Inserted { .. })),
        "a move to another phase lowers the run again"
    );
}

#[test]
fn trim_evicts_only_tiles_no_lowering_holds() {
    let mut slice = slice();
    line(&mut slice, 1, 0, "Kept", true);
    assert!(slice.draw());
    let held = slice.compositor.memory();
    assert!(held.atlas_pinned_bytes > 0);
    assert_eq!(
        slice.compositor.trim(0),
        0,
        "a page with pinned tiles stays"
    );
    assert_eq!(
        slice.compositor.memory().atlas_pinned_bytes,
        held.atlas_pinned_bytes
    );

    let mut tx = slice.tx();
    tx.remove_chunk(ChunkKey::new(1, 0));
    tx.splice(slice.world.root, 0, 1, &[]);
    slice.world.entries -= 1;
    slice.compositor.commit(tx).expect("the removal commits");
    assert!(slice.draw());
    assert!(slice.compositor.trim(0) > 0);
    assert_eq!(slice.compositor.memory().atlas_bytes, 0);
    line(&mut slice, 2, 0, "Again", true);
    assert!(slice.draw());
    assert!(
        slice
            .frame
            .resources
            .pages
            .iter()
            .any(|op| matches!(op, PageOp::Destroy { .. })),
        "the trimmed page is destroyed in the next frame"
    );
}

#[test]
fn a_budget_is_enforced_when_a_frame_is_built() {
    let mut slice = slice();
    line(&mut slice, 1, 0, "Budget", true);
    assert!(slice.draw());
    let mut tx = slice.tx();
    tx.remove_chunk(ChunkKey::new(1, 0));
    tx.splice(slice.world.root, 0, 1, &[]);
    slice.compositor.commit(tx).expect("the removal commits");
    slice.compositor.set_budgets(CompositorBudgets {
        atlas_bytes: Some(0),
    });
    assert!(slice.draw());
    assert_eq!(slice.compositor.memory().atlas_bytes, 0);
    assert!(
        slice
            .frame
            .resources
            .pages
            .iter()
            .any(|op| matches!(op, PageOp::Destroy { .. }))
    );
}

#[test]
fn forgetting_the_caches_destroys_every_page_and_starts_over() {
    let mut slice = slice();
    line(&mut slice, 1, 0, "Forgotten", true);
    assert!(slice.draw());
    let freed = slice.compositor.forget_caches();
    assert!(freed > 0);
    let memory = slice.compositor.memory();
    assert_eq!((memory.atlas_bytes, memory.atlas_pinned_bytes), (0, 0));

    assert!(slice.draw());
    let pages = &slice.frame.resources.pages;
    let destroyed = pages
        .iter()
        .position(|op| matches!(op, PageOp::Destroy { .. }));
    let created = pages
        .iter()
        .rposition(|op| matches!(op, PageOp::Create { .. }));
    assert!(
        destroyed.is_some() && destroyed < created,
        "old pages go before new ones come"
    );
    assert!(matches!(
        slice.frame.chunk_notes.first(),
        Some(ChunkNote::ResetAll)
    ));
    assert!(slice.frame.damage.draw.is_full());
}

#[test]
fn subpixel_sprites_stay_under_a_translucent_paint() {
    let caps = RenderCapabilities {
        subpixel_text: true,
        ..FULL
    };
    let mut slice = Slice::new(CaptureRenderer::with_capabilities(caps));
    slice.subpixel = Some(SubpixelOrder::RedToBlue);
    slice.draw();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let paint = world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
    let font = world.font;
    let key = ChunkKey::new(1, 0);
    let mut chunk = tx.record(key, props(world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    let flags = GlyphFlags::SUBPIXEL;
    run_with(&mut chunk, font, "Stripes", 12.0, [4.0, 20.0], fill, flags);
    world.append(&mut tx, OrderEntry::Chunk(key));
    slice.compositor.commit(tx).expect("the run commits");
    assert!(slice.draw());
    assert!(!slice.frame.lanes.subpixel.sources.is_empty());
    assert!(slice.frame.lanes.mono.sources.is_empty());

    let mut tx = slice.tx();
    tx.set_paint(paint, Paint::Solid(Color([0.0, 0.0, 0.0, 0.5])));
    slice.compositor.commit(tx).expect("the paint commits");
    assert!(slice.draw());
    assert!(
        !slice.frame.lanes.subpixel.sources.is_empty(),
        "a translucent paint draws subpixel sprites"
    );
    assert!(slice.frame.lanes.mono.sources.is_empty());
}

#[test]
fn a_recovered_renderer_gets_every_page_again() {
    let mut slice = slice();
    line(&mut slice, 1, 0, "Lost", true);
    assert!(slice.draw());
    line(&mut slice, 2, 1, "Found", true);
    assert_eq!(slice.build_only(), Built::Frame);
    slice
        .validator
        .check(&slice.frame)
        .expect("the frame is valid");
    slice.renderer.lose_device();
    slice
        .compositor
        .frame_done(slice.output, slice.frame.id, &FrameOutcome::Recovered);
    slice.validator.device_lost();
    assert!(slice.draw());
    let pages = &slice.frame.resources.pages;
    let created = pages
        .iter()
        .position(|op| matches!(op, PageOp::Create { .. }));
    let uploaded = pages
        .iter()
        .position(|op| matches!(op, PageOp::Upload { .. }));
    assert!(created.is_some() && created < uploaded);
    assert!(!pages.iter().any(|op| matches!(op, PageOp::Destroy { .. })));
    assert!(matches!(
        slice.frame.chunk_notes.first(),
        Some(ChunkNote::ResetAll)
    ));
}

/// Builds a frame of `output` for `renderer`, checks it with `validator`, and draws it.
///
/// Returns whether a frame was built.
fn draw_output(
    slice: &mut Captured,
    output: iceblink::OutputId,
    renderer: &mut CaptureRenderer,
    validator: &mut FrameValidator,
    frame: &mut Frame,
) -> bool {
    let caps = renderer.capabilities();
    let request = FrameRequest {
        caps: &caps,
        age: BufferAge::Kept,
        clock: FrameClock::default(),
        subpixel: None,
    };
    if slice.compositor.build_frame(output, &request, frame) != Built::Frame {
        return false;
    }
    if let Err(violations) = validator.check(frame) {
        panic!(
            "frame {} breaks the frame contract:\n{}",
            frame.id.0,
            describe(&violations)
        );
    }
    let outcome = renderer.draw(frame);
    slice.compositor.frame_done(output, frame.id, &outcome);
    true
}

#[test]
fn a_second_output_joining_late_gets_every_page_without_rasterising_again() {
    let mut slice = slice();
    line(&mut slice, 1, 0, "Shared glyphs", true);
    assert!(slice.draw());
    assert!(!slice.draw());
    let rasterised = stat(&slice.compositor, "content.glyphs_rasterised");

    let second = slice.compositor.create_output(OutputConfig {
        size: [SIDE, SIDE],
        scale: 1.0,
        opaque: true,
    });
    let mut renderer = CaptureRenderer::with_capabilities(FULL);
    renderer.configure(RenderTarget {
        size: [SIDE, SIDE],
        scale: 1.0,
        opaque: true,
    });
    let mut validator = FrameValidator::new();
    validator.set_capabilities(Some(FULL));
    let mut frame = Frame::default();
    let mut tx = slice.tx();
    let root = tx.create_order(OrderNode::default());
    let font = slice.world.font;
    let paint = slice.world.paints[0];
    let key = ChunkKey::new(9, 0);
    let mut chunk = tx.record(key, props(slice.world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    run(&mut chunk, font, "Shared glyphs", 12.0, [4.0, 12.0], fill);
    tx.splice(root, 0, 0, &[OrderEntry::Chunk(key)]);
    tx.set_root(second, Some(root));
    slice
        .compositor
        .commit(tx)
        .expect("the second root commits");

    assert!(draw_output(
        &mut slice,
        second,
        &mut renderer,
        &mut validator,
        &mut frame
    ));
    let pages = &frame.resources.pages;
    assert!(pages.iter().any(|op| matches!(op, PageOp::Create { .. })));
    assert!(pages.iter().any(|op| matches!(op, PageOp::Upload { .. })));
    assert!(!frame.lanes.mono.sources.is_empty());
    assert_eq!(
        stat(&slice.compositor, "content.glyphs_rasterised"),
        rasterised,
        "the new renderer gets the glyphs rasterised for the first"
    );

    line(&mut slice, 1, 0, "Shared glyphs!", false);
    assert!(slice.draw(), "the first output draws again");
    assert!(
        !slice
            .frame
            .resources
            .pages
            .iter()
            .any(|op| matches!(op, PageOp::Destroy { .. })),
        "the first output keeps its pages"
    );

    let mut tx = slice.tx();
    let mut chunk = tx.record(key, props(slice.world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    run(&mut chunk, font, "Shared glyphs!", 12.0, [4.0, 12.0], fill);
    slice
        .compositor
        .commit(tx)
        .expect("the second root's line commits");
    assert!(draw_output(
        &mut slice,
        second,
        &mut renderer,
        &mut validator,
        &mut frame
    ));
    assert!(
        frame
            .resources
            .pages
            .iter()
            .any(|op| matches!(op, PageOp::Upload { .. })),
        "the glyph the first output added reaches the second"
    );
    assert_eq!(
        renderer.mirror().pages,
        slice.renderer.mirror().pages,
        "both renderers hold the same pages"
    );
}

#[test]
fn an_idle_output_catches_up_on_resource_work() {
    let mut slice = slice();
    line(&mut slice, 1, 0, "Busy", true);
    assert!(slice.draw());
    let second = slice.compositor.create_output(OutputConfig {
        size: [SIDE, SIDE],
        scale: 1.0,
        opaque: true,
    });
    let mut renderer = CaptureRenderer::with_capabilities(FULL);
    renderer.configure(RenderTarget {
        size: [SIDE, SIDE],
        scale: 1.0,
        opaque: true,
    });
    let mut validator = FrameValidator::new();
    validator.set_capabilities(Some(FULL));
    let mut frame = Frame::default();
    let mut tx = slice.tx();
    let root = tx.create_order(OrderNode::default());
    tx.set_root(second, Some(root));
    slice
        .compositor
        .commit(tx)
        .expect("the second root commits");
    assert!(draw_output(
        &mut slice,
        second,
        &mut renderer,
        &mut validator,
        &mut frame
    ));

    let mut caught_up = None;
    for step in 0..100 {
        line(&mut slice, 1, 0, &format!("Busy {step}"), false);
        assert!(slice.draw());
        if draw_output(
            &mut slice,
            second,
            &mut renderer,
            &mut validator,
            &mut frame,
        ) {
            caught_up = Some(step);
            break;
        }
    }
    let step = caught_up.expect("the idle output gets a frame");
    assert!(step > 10, "an idle output is not woken on every frame");
    assert!(frame.damage.draw.is_empty());
    assert!(
        frame
            .resources
            .pages
            .iter()
            .any(|op| matches!(op, PageOp::Upload { .. })),
        "the frame carries the uploads made for the busy output"
    );
}

#[test]
fn subpixel_glyphs_are_rasterised_once_at_commit() {
    let rasterised = |subpixel: Option<SubpixelOrder>| {
        let caps = RenderCapabilities {
            subpixel_text: true,
            ..FULL
        };
        let mut slice = Slice::new(CaptureRenderer::with_capabilities(caps));
        slice.subpixel = subpixel;
        slice.draw();
        let mut tx = slice.tx();
        let world = &mut slice.world;
        let paint = world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
        let font = world.font;
        let key = ChunkKey::new(1, 0);
        let mut chunk = tx.record(key, props(world.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        let flags = GlyphFlags::SUBPIXEL;
        run_with(&mut chunk, font, "Stripes", 12.0, [4.0, 20.0], fill, flags);
        world.append(&mut tx, OrderEntry::Chunk(key));
        slice.compositor.commit(tx).expect("the run commits");
        stat(&slice.compositor, "content.glyphs_rasterised")
    };
    let mono = rasterised(None);
    assert!(mono > 0);
    assert_eq!(rasterised(Some(SubpixelOrder::RedToBlue)), mono);
}

#[test]
fn glyphs_are_rasterised_in_parallel_after_the_caches_start_over() {
    let mut slice = slice();
    let text: String = (0x21_u8..0x7f).map(char::from).collect();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let paint = world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
    let font = world.font;
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    for size in [10.0, 11.0, 12.0] {
        run(&mut chunk, font, &text, size, [0.0, size * 4.0], fill);
    }
    world.append(&mut tx, OrderEntry::Chunk(ChunkKey::new(1, 0)));
    slice.compositor.commit(tx).expect("the lines commit");
    assert!(slice.draw());

    slice.compositor.forget_caches();
    let batches = stat(&slice.compositor, "content.prefetch_batches");
    assert!(slice.draw());
    assert!(stat(&slice.compositor, "content.prefetch_batches") > batches);
}

#[test]
fn a_run_recorded_at_an_offset_has_the_ink_of_the_run_moved_by_it() {
    let damage = |offset: [f32; 2], origin: [f32; 2]| {
        let mut slice = slice();
        let mut tx = slice.tx();
        let world = &mut slice.world;
        let paint = world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
        let font = world.font;
        let key = ChunkKey::new(1, 0);
        let mut chunk = tx.record(key, props(world.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        run(&mut chunk, font, "Offset", 12.0, origin, fill);
        tx.set_offset(key, offset);
        world.append(&mut tx, OrderEntry::Chunk(key));
        slice.compositor.commit(tx).expect("the run commits");
        assert!(slice.draw());
        let mut tx = slice.tx();
        tx.remove_chunk(key);
        slice.compositor.commit(tx).expect("the removal commits");
        assert!(slice.draw());
        slice.frame.damage.draw.rects().to_vec()
    };
    assert_eq!(
        damage([7.0, 3.0], [4.0, 20.0]),
        damage([0.0, 0.0], [11.0, 23.0])
    );
}

#[test]
fn a_large_image_reaches_the_renderer_with_levels_that_follow_its_writes() {
    let mut slice = slice();
    let mut tx = slice.tx();
    let image = tx.create_image(ImageDesc {
        width: 600,
        height: 40,
        format: ImageFormat::RGBA8_PREMULTIPLIED,
    });
    let write = |tx: &mut iceblink::Transaction, rect: [i32; 4], value: u8| {
        let bytes = vec![value; (rect[2] * rect[3] * 4) as usize];
        tx.write_image(
            image,
            ImageWrite {
                rect: Rect::new(Point::new(rect[0], rect[1]), Size::new(rect[2], rect[3])),
                bytes: std::sync::Arc::from(bytes),
                offset: 0,
                stride: rect[2] as u32 * 4,
            },
        );
    };
    write(&mut tx, [0, 0, 600, 40], 200);
    let key = ChunkKey::new(5, 0);
    let mut chunk = tx.record(key, props(slice.world.space));
    let slot = chunk.image_slot(image);
    chunk.image(Image {
        bounds: [0.0, 0.0, 60.0, 4.0],
        frame: [0.0, 0.0, 60.0, 4.0],
        source: [0.0, 0.0, 600.0, 40.0],
        radii: [0.0; 8],
        shapes: ROUND,
        paint: PaintRef::NONE,
        image: slot,
        opacity: 1.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: ImageFlags::default(),
        extend: [Extend::NONE; 2],
        filter: ImageFilter::LINEAR,
    });
    slice.world.append(&mut tx, OrderEntry::Chunk(key));
    slice.compositor.commit(tx).expect("the image commits");
    assert!(slice.draw());

    let transcript = slice.renderer.transcript().expect("a frame was drawn");
    assert!(
        transcript.as_str().contains("levels=10"),
        "{}",
        transcript.as_str()
    );
    let page = slice
        .renderer
        .mirror()
        .pages
        .values()
        .find(|page| page.size == [600, 40])
        .expect("the image has a page of its own");
    assert_eq!(page.below.len(), 9);
    assert_eq!(
        page.level(3).and_then(|level| level.texel(0, 0)),
        Some(&[200; 4][..])
    );

    let mut tx = slice.tx();
    write(&mut tx, [0, 0, 8, 8], 0);
    slice.compositor.commit(tx).expect("the write commits");
    assert!(slice.draw());
    let page = slice
        .renderer
        .mirror()
        .pages
        .values()
        .find(|page| page.size == [600, 40])
        .expect("the image keeps its page");
    assert_eq!(
        page.level(3).and_then(|level| level.texel(0, 0)),
        Some(&[0; 4][..])
    );
    assert_eq!(
        page.level(3).and_then(|level| level.texel(1, 0)),
        Some(&[200; 4][..])
    );
}
