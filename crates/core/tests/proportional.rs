//! Frame and commit work that stays proportional to what changed, shown by the work counters.

mod common;

use std::sync::Arc;

use iceblink::Compositor;
use iceblink::kurbo::{Affine, BezPath};
use iceblink::{
    ChunkKey, ClipRef, Color, FillRule, Matrix4, OrderEntry, Paint, PaintKind, PathFill, PathItem,
    RenderCapabilities, ScrollNode, Shift, SpaceRef, Transaction, TransformNode,
};
use iceblink_testing::capture::CaptureRenderer;

use common::list::list;
use common::slice::Slice;
use common::stories;
use common::text::run;
use common::{FULL, identity, props, rect, stat};

type Captured = Slice<CaptureRenderer>;

/// Returns a slice past its first frame.
fn slice() -> Captured {
    let mut slice = Slice::new(CaptureRenderer::with_capabilities(FULL));
    slice.draw();
    slice
}

/// Returns how much the work counter `name` grows while `then` runs.
fn work(slice: &mut Captured, name: &str, then: impl FnOnce(&mut Captured)) -> u64 {
    let before = stat(&slice.compositor, name);
    then(slice);
    stat(&slice.compositor, name) - before
}

/// Records chunk 1 with `lines` lines of text, then chunk 2 with a quad over all of them, whose
/// paint it returns.
fn text_under_a_quad(slice: &mut Captured, lines: u32) -> iceblink::PaintId {
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let ink = world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
    let font = world.font;
    let key = ChunkKey::new(1, 0);
    let mut chunk = tx.record(key, props(world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, ink);
    for line in 0..lines {
        let baseline = 12.0 + line as f32 * 14.0;
        run(&mut chunk, font, "every line", 12.0, [4.0, baseline], fill);
    }
    world.append(&mut tx, OrderEntry::Chunk(key));
    let cover = world.solid(&mut tx, [1.0, 0.0, 0.0, 1.0]);
    world.boxes(
        &mut tx,
        ChunkKey::new(2, 0),
        cover,
        &[[0.0, 0.0, 100.0, 100.0]],
        true,
    );
    slice.compositor.commit(tx).expect("the scene commits");
    assert!(slice.draw());
    cover
}

#[test]
fn a_lowering_is_checked_in_full_only_after_a_change_that_can_move_its_glyphs() {
    let mut slice = slice();
    let cover = text_under_a_quad(&mut slice, 6);

    let checked = work(&mut slice, "work.lowerings_checked", |slice| {
        for frame in 0..4 {
            let mut tx = slice.tx();
            let red = if frame % 2 == 0 { 0.0 } else { 1.0 };
            tx.set_paint(cover, Paint::Solid(Color([red, 1.0, 0.0, 1.0])));
            slice.compositor.commit(tx).expect("the paint commits");
            assert!(slice.draw());
        }
    });
    assert_eq!(
        checked, 0,
        "a paint write that keeps the runs opaque leaves the text where it was"
    );
}

#[test]
fn a_frame_path_moved_by_a_translation_is_drawn_moved_without_lowering() {
    let mut slice = slice();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let scaled = tx.create_transform(iceblink::TransformNode {
        local: Matrix4::scale(2.0, 2.0, 1.0),
        ..identity(Some(world.space))
    });
    let space = tx.create_transform(identity(Some(scaled)));
    let ramp = tx.create_paint(stories::ramp());
    let key = ChunkKey::new(1, 0);
    let mut chunk = tx.record(key, props(space));
    let fill = chunk.paint_ref(PaintKind::GRADIENT, ramp);
    let mut wave = BezPath::new();
    wave.move_to((8.0, 60.0));
    wave.curve_to((30.0, 10.0), (60.0, 90.0), (90.0, 40.0));
    wave.line_to((90.0, 90.0));
    wave.close_path();
    chunk.path(PathItem {
        path: Arc::new(wave),
        placement: Affine::IDENTITY,
        brush: Affine::IDENTITY,
        fill: Some(PathFill {
            paint: fill,
            rule: FillRule::NON_ZERO,
        }),
        stroke: None,
        clips: Arc::from(Vec::new()),
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    });
    world.append(&mut tx, OrderEntry::Chunk(key));
    slice.compositor.commit(tx).expect("the path commits");
    assert!(slice.draw());

    let lowered = work(&mut slice, "work.chunks_lowered", |slice| {
        let mut tx = slice.tx();
        tx.set_offset(key, [0.5, 3.25]);
        slice.compositor.commit(tx).expect("the offset commits");
        assert!(slice.draw());
        let mut tx = slice.tx();
        tx.set_transform(space, Matrix4::translation(6.0, -2.0, 0.0));
        slice.compositor.commit(tx).expect("the transform commits");
        assert!(slice.draw());
    });
    assert_eq!(lowered, 0);
    // A fresh lowering would place the path at scale(2) · translate(6, -2) · translate(0.5, 3.25).
    let moved = slice.frame.paths[0].transform.as_coeffs();
    assert_eq!(moved, [2.0, 0.0, 0.0, 2.0, 13.0, 2.5]);
}

#[test]
fn local_orders_are_kept_while_a_chunk_is_redrawn_and_moved_by_whole_pixels() {
    let mut slice = slice();
    let cover = text_under_a_quad(&mut slice, 6);

    let computed = work(&mut slice, "work.local_orders", |slice| {
        for frame in 0..4 {
            let mut tx = slice.tx();
            let red = if frame % 2 == 0 { 0.0 } else { 1.0 };
            tx.set_paint(cover, Paint::Solid(Color([red, 1.0, 0.0, 1.0])));
            tx.set_offset(ChunkKey::new(1, 0), [frame as f32, 2.0]);
            slice
                .compositor
                .commit(tx)
                .expect("the frame's changes commit");
            assert!(slice.draw());
        }
    });
    assert_eq!(computed, 0);
}

#[test]
fn a_new_glyph_run_is_lowered_once_for_its_ink_and_its_first_frame() {
    let mut slice = slice();
    let at_commit = work(&mut slice, "content.glyphs_placed", |slice| {
        let mut tx = slice.tx();
        let world = &mut slice.world;
        let paint = world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
        let font = world.font;
        let key = ChunkKey::new(1, 0);
        let mut chunk = tx.record(key, props(world.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        run(&mut chunk, font, "placed once", 12.0, [4.0, 40.0], fill);
        world.append(&mut tx, OrderEntry::Chunk(key));
        slice.compositor.commit(tx).expect("the line commits");
    });
    let at_frame = work(&mut slice, "content.glyphs_placed", |slice| {
        assert!(slice.draw());
    });
    assert!(at_commit > 0);
    assert_eq!(
        at_frame, 0,
        "the frame takes the run as the commit lowered it"
    );
}

#[test]
fn an_identical_recording_takes_the_measured_glyph_inks() {
    let mut slice = slice();
    let key = ChunkKey::new(1, 0);
    let mut tx = slice.tx();
    let paint = slice.world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
    slice.world.append(&mut tx, OrderEntry::Chunk(key));
    let record = |slice: &mut Captured, mut tx: Transaction| {
        let mut chunk = tx.record(key, props(slice.world.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        let font = slice.world.font;
        run(&mut chunk, font, "measured once", 12.0, [4.0, 40.0], fill);
        slice.compositor.commit(tx).expect("the line commits");
    };
    record(&mut slice, tx);
    assert!(slice.draw());

    let placed = work(&mut slice, "content.glyphs_placed", |slice| {
        let tx = slice.tx();
        record(slice, tx);
    });
    assert_eq!(slice.compositor.counters().commit.chunks_identical, 1);
    assert_eq!(placed, 0, "the runs are not lowered again to measure them");
}

#[test]
fn a_glyph_run_recorded_below_the_output_is_drawn_as_the_commit_lowered_it() {
    let mut slice = slice();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let paint = world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
    let font = world.font;
    let scroll = tx.create_scroll(ScrollNode {
        space: world.space,
        port: [0.0, 0.0, 128.0, 128.0],
        content: [128.0, 512.0],
        offset: [0.0, 0.0],
        behaviour: None,
    });
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(world.space))
    });
    let key = ChunkKey::new(1, 0);
    let mut chunk = tx.record(key, props(content));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    run(&mut chunk, font, "scrolled in", 12.0, [4.0, 180.0], fill);
    world.append(&mut tx, OrderEntry::Chunk(key));
    slice.compositor.commit(tx).expect("the line commits");
    slice.draw();

    let placed = work(&mut slice, "content.glyphs_placed", |slice| {
        let mut tx = slice.tx();
        tx.scroll_to(scroll, [0.0, 100.0]);
        slice.compositor.commit(tx).expect("the scroll commits");
        assert!(slice.draw());
    });
    assert_eq!(
        placed, 0,
        "the frame takes the run as the commit lowered it"
    );
}

#[test]
fn outlined_glyphs_recorded_below_the_output_are_drawn_where_they_scroll_in() {
    let mut slice = slice();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let paint = world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
    let font = world.font;
    let scroll = tx.create_scroll(ScrollNode {
        space: world.space,
        port: [0.0, 0.0, 128.0, 128.0],
        content: [128.0, 512.0],
        offset: [0.0, 0.0],
        behaviour: None,
    });
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(world.space))
    });
    let key = ChunkKey::new(1, 0);
    let mut chunk = tx.record(key, props(content));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    // Glyphs this large are drawn as outlines.
    run(&mut chunk, font, "Ag", 100.0, [4.0, 220.0], fill);
    world.append(&mut tx, OrderEntry::Chunk(key));
    slice.compositor.commit(tx).expect("the line commits");
    slice.draw();

    let mut tx = slice.tx();
    tx.scroll_to(scroll, [0.0, 120.0]);
    slice.compositor.commit(tx).expect("the scroll commits");
    assert!(slice.draw());
    assert!(
        !slice.frame.paths.is_empty(),
        "the outlines are drawn where the scroll brought them"
    );
    for path in &slice.frame.paths {
        assert!(path.ink.origin.y < 128, "{:?} lies on the output", path.ink);
    }
}

#[test]
fn a_glyph_run_lowering_is_kept_only_where_an_output_draws_its_client() {
    let mut slice = slice();
    let (client, _) = slice.compositor.register_client();
    let font = slice.world.font;
    let mut bytes = |origin: [f32; 2], key: u64| {
        let before = slice.compositor.memory().chunk_bytes;
        let mut tx = slice.compositor.transaction(client);
        let space = tx.create_transform(identity(None));
        let paint = tx.create_paint(Paint::Solid(Color([0.0, 0.0, 0.0, 1.0])));
        let mut chunk = tx.record(ChunkKey::new(key, 0), props(space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        run(&mut chunk, font, "undrawn", 12.0, origin, fill);
        slice.compositor.commit(tx).expect("the line commits");
        slice.compositor.memory().chunk_bytes - before
    };
    let inside = bytes([4.0, 40.0], 1);
    let outside = bytes([4000.0, 40.0], 2);
    assert_eq!(
        inside, outside,
        "another client's output keeps no lowering of the line"
    );
}

#[test]
fn a_renderer_reset_releases_the_pins_only_it_held() {
    let mut slice = slice();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let ink = world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
    let font = world.font;
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, ink);
    run(&mut chunk, font, "pinned", 12.0, [4.0, 12.0], fill);
    world.append(&mut tx, OrderEntry::Chunk(ChunkKey::new(1, 0)));
    slice.compositor.commit(tx).expect("the line commits");
    assert!(slice.draw());
    assert!(slice.compositor.memory().atlas_pinned_bytes > 0);

    let mut tx = slice.tx();
    tx.set_paint(ink, Paint::Solid(Color([0.0, 0.0, 1.0, 1.0])));
    slice.compositor.commit(tx).expect("the paint commits");
    assert_eq!(slice.build_only(), iceblink::Built::Frame);
    let id = slice.frame.id;
    slice
        .compositor
        .frame_done(slice.output, id, &iceblink::FrameOutcome::Recovered);
    assert_eq!(
        slice.compositor.memory().atlas_pinned_bytes,
        0,
        "no renderer holds the line resident"
    );
}

#[test]
fn tiles_stay_pinned_only_while_the_last_frame_drew_them_without_resident_chunks() {
    let caps = RenderCapabilities {
        resident_chunks: false,
        ..FULL
    };
    let mut slice = Slice::new(CaptureRenderer::with_capabilities(caps));
    slice.draw();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let ink = world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
    let font = world.font;
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, ink);
    run(&mut chunk, font, "pinned", 12.0, [4.0, 12.0], fill);
    world.append(&mut tx, OrderEntry::Chunk(ChunkKey::new(1, 0)));
    let other = world.solid(&mut tx, [1.0, 0.0, 0.0, 1.0]);
    world.boxes(
        &mut tx,
        ChunkKey::new(2, 0),
        other,
        &[[0.0, 100.0, 20.0, 20.0]],
        true,
    );
    slice.compositor.commit(tx).expect("the scene commits");
    assert!(slice.draw());
    assert!(slice.compositor.memory().atlas_pinned_bytes > 0);

    let mut tx = slice.tx();
    tx.set_paint(other, Paint::Solid(Color([0.0, 1.0, 0.0, 1.0])));
    slice.compositor.commit(tx).expect("the paint commits");
    assert!(slice.draw());
    assert_eq!(slice.compositor.counters().frame.chunks_drawn, 1);
    assert_eq!(
        slice.compositor.memory().atlas_pinned_bytes,
        0,
        "the last frame drew no glyph"
    );

    assert!(
        slice.compositor.trim(0) > 0,
        "the glyph tiles can be evicted"
    );
    let mut tx = slice.tx();
    tx.set_paint(ink, Paint::Solid(Color([0.0, 0.0, 1.0, 1.0])));
    slice.compositor.commit(tx).expect("the paint commits");
    let lowered = work(&mut slice, "work.chunks_lowered", |slice| {
        assert!(slice.draw())
    });
    assert_eq!(
        lowered, 1,
        "the line lost its tiles, so it is lowered again"
    );
    assert!(!slice.frame.lanes.mono.items.is_empty());
}

#[test]
fn a_new_rasteriser_measures_the_glyph_inks_again() {
    let mut slice = slice();
    slice
        .compositor
        .use_glyph_raster(Arc::new(iceblink_testing::fakes::FakeGlyphRaster::new()));
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let under = world.solid(&mut tx, [1.0, 0.0, 0.0, 1.0]);
    world.boxes(
        &mut tx,
        ChunkKey::new(1, 0),
        under,
        &[[30.0, 35.0, 8.0, 8.0]],
        true,
    );
    let ink = world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
    let font = world.font;
    let mut chunk = tx.record(ChunkKey::new(2, 0), props(world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, ink);
    // The fake draws a box 20 pixels wide, and Noto Sans a W twice as wide.
    run(&mut chunk, font, "W", 40.0, [4.0, 60.0], fill);
    world.append(&mut tx, OrderEntry::Chunk(ChunkKey::new(2, 0)));
    slice.compositor.commit(tx).expect("the scene commits");
    assert!(slice.draw());

    slice
        .compositor
        .use_glyph_raster(Arc::new(iceblink_text::SwashRaster::new()));
    assert!(slice.draw());
    let mut tx = slice.tx();
    tx.set_paint(under, Paint::Solid(Color([0.0, 1.0, 0.0, 1.0])));
    slice.compositor.commit(tx).expect("the paint commits");
    assert!(slice.draw());
    assert_eq!(
        slice.compositor.counters().frame.chunks_drawn,
        2,
        "the W is drawn again over the quad"
    );
}

/// The work of three scrolls of a list by one row.
#[derive(Debug, PartialEq)]
struct Scrolled {
    /// Inks computed by the commits.
    inks: u64,
    /// Order node entries the commits summed.
    entries: u64,
    /// Items the frames culled.
    culled: u32,
}

/// Returns the work of a list of `rows` rows, `faded` as `list` makes them, scrolling by one row
/// three times, each frame shifting and drawing the backing and the row that enters.
fn scrolled(rows: u32, faded: bool) -> Scrolled {
    let mut slice = slice();
    let scroll = list(&mut slice, rows, faded).scroll;
    let (mut inks, mut entries, mut culled) = (0, 0, 0);
    for step in 1..=3 {
        inks += work(&mut slice, "work.inks_computed", |slice| {
            entries += work(slice, "work.entries_summed", |slice| {
                let mut tx = slice.tx();
                tx.scroll_to(scroll, [0.0, step as f32 * 10.0]);
                slice.compositor.commit(tx).expect("the scroll commits");
            });
        });
        assert!(slice.draw());
        assert_eq!(
            slice.frame.shifts.as_slice(),
            &[Shift {
                region: rect(0, 0, 100, 100),
                by: [0, -10],
            }]
        );
        let frame = slice.compositor.counters().frame;
        if !faded {
            assert_eq!(frame.chunks_drawn, 2, "the backing and the row that enters");
        }
        culled += frame.items_culled;
    }
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
    Scrolled {
        inks,
        entries,
        culled,
    }
}

#[test]
fn a_whole_pixel_scroll_computes_no_ink_however_long_the_list() {
    let short = scrolled(20, false);
    let long = scrolled(400, false);
    assert_eq!(short.inks, 0, "no row's ink is computed again");
    assert_eq!(long.inks, short.inks);
    assert_eq!(long.entries, short.entries, "no list entry is summed again");
}

#[test]
fn a_whole_pixel_scroll_of_a_list_that_covers_its_port_sums_no_order_node() {
    let mut slice = slice();
    let scroll = list(&mut slice, 400, false).scroll;
    let scroll_to = |slice: &mut Captured, offset: f32| {
        let mut tx = slice.tx();
        tx.scroll_to(scroll, [0.0, offset]);
        slice.compositor.commit(tx).expect("the scroll commits");
        assert!(slice.draw());
    };
    // The first scroll moves the top row's gap out of the port.
    scroll_to(&mut slice, 10.0);
    let summed = work(&mut slice, "work.entries_summed", |slice| {
        scroll_to(slice, 20.0);
        scroll_to(slice, 30.0);
    });
    assert_eq!(summed, 0, "the list's device ink stays the port");
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_list_that_moves_its_first_row_to_its_end_sums_only_the_moved_row() {
    let mut slice = slice();
    let list = list(&mut slice, 400, false);
    let summed = work(&mut slice, "work.entries_summed", |slice| {
        let mut tx = slice.tx();
        let key = list.row(&mut tx, 400);
        tx.splice(list.node, 0, 1, &[]);
        tx.splice(list.node, 399, 0, &[OrderEntry::Chunk(key)]);
        slice
            .compositor
            .commit(tx)
            .expect("the recycled row commits");
        assert!(slice.draw());
    });
    assert_eq!(summed, 3, "the new row, and the root's two entries");
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_list_that_moves_a_row_while_a_kept_group_changes_keeps_every_ink_exact() {
    let mut slice = slice();
    let list = list(&mut slice, 40, true);
    let summed = work(&mut slice, "work.entries_summed", |slice| {
        let mut tx = slice.tx();
        let key = list.row(&mut tx, 40);
        tx.splice(list.node, 1, 1, &[]);
        tx.splice(list.node, 39, 0, &[OrderEntry::Chunk(key)]);
        // Row 8 sits in a faded group, which the splices keep and whose ink this changes.
        tx.set_offset(ChunkKey::new(2, 8), [2.0, 0.0]);
        slice
            .compositor
            .commit(tx)
            .expect("the recycled row commits");
        assert!(slice.draw());
    });
    assert_eq!(
        summed, 5,
        "the new row, the group, its row in the list and the root's two entries"
    );
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_shifted_frame_looks_only_at_the_rows_its_damage_meets() {
    let short = scrolled(20, false);
    let long = scrolled(400, false);
    assert_eq!(long.culled, short.culled);
}

#[test]
fn a_shifted_frame_damaged_at_both_ends_of_the_port_looks_only_at_the_rows_each_end_meets() {
    let mut slice = slice();
    let scroll = list(&mut slice, 400, false).scroll;
    let mut tx = slice.tx();
    tx.scroll_to(scroll, [0.0, 10.0]);
    tx.set_offset(ChunkKey::new(2, 1), [1.0, 0.0]);
    slice.compositor.commit(tx).expect("the scroll commits");
    assert!(slice.draw());

    assert_eq!(slice.frame.shifts.len(), 1);
    let culled = slice.compositor.counters().frame.items_culled;
    assert_eq!(culled, 0, "no row between the two ends is looked at");
}

#[test]
fn a_whole_pixel_scroll_of_rows_in_faded_groups_sums_no_more_entries_however_long_the_list() {
    let short = scrolled(20, true);
    let long = scrolled(400, true);
    assert_eq!(long.inks, 0, "no row's ink is computed again");
    assert_eq!(long.entries, short.entries, "no list entry is summed again");
}

#[test]
fn a_whole_pixel_scroll_measures_the_inks_of_the_rows_only_for_a_frame_that_refuses_the_shift() {
    let mut slice = slice();
    let scroll = list(&mut slice, 400, false).scroll;
    let mut scroll_and_draw = |offset: f32, caps: RenderCapabilities| {
        work(&mut slice, "work.moving_inks", |slice| {
            let mut tx = slice.tx();
            tx.scroll_to(scroll, [0.0, offset]);
            slice.compositor.commit(tx).expect("the scroll commits");
            assert!(slice.draw_with(caps));
        })
    };
    assert_eq!(
        scroll_and_draw(10.0, FULL),
        0,
        "a shifted frame measures nothing"
    );
    assert!(
        scroll_and_draw(20.0, RenderCapabilities::MINIMAL) > 0,
        "a frame that cannot shift measures the rows it damages"
    );
}
