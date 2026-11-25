//! Allocations of small commits once a client's buffers are warm, counted on the test's thread.

mod common;

use iceblink::Compositor;
use iceblink::{Built, ChunkKey, Color, OrderEntry, Paint, PaintId, PaintKind, ScrollId};
use iceblink_testing::capture::CaptureRenderer;
use iceblink_testing::{CountingAlloc, allocations};

use common::list::list;
use common::slice::Slice;
use common::{FULL, props, quad};

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

type Captured = Slice<CaptureRenderer>;

/// Commits five bars of a progress meter, the first `filled` wide, and a write of `paint`.
///
/// On the `first` turn the bars are appended to the root.
fn turn(slice: &mut Captured, paint: PaintId, filled: f32, first: bool) {
    let mut tx = slice.tx();
    let world = &mut slice.world;
    for bar in 0..5 {
        let key = ChunkKey::new(7, bar);
        let mut chunk = tx.record(key, props(world.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        let width = if bar == 0 { filled } else { 20.0 };
        chunk.quad(quad([bar as f32 * 24.0, 10.0, width, 12.0], fill));
        if first {
            world.append(&mut tx, OrderEntry::Chunk(key));
        }
    }

    let shade = filled / 100.0;
    tx.set_paint(paint, Paint::Solid(Color([0.2, shade, 0.4, 1.0])));
    slice.compositor.commit(tx).expect("the turn commits");
}

#[test]
fn a_warm_turn_that_rerecords_a_meter_commits_without_allocating() {
    let mut slice = Slice::new(CaptureRenderer::with_capabilities(FULL));
    slice.compositor.set_verification(false);
    let mut tx = slice.tx();
    let paint = slice.world.solid(&mut tx, [0.2, 0.0, 0.4, 1.0]);
    slice.compositor.commit(tx).expect("the paint commits");
    turn(&mut slice, paint, 10.0, true);
    assert!(slice.draw());
    for step in 1..6 {
        turn(&mut slice, paint, 10.0 + step as f32, false);
        assert!(slice.draw());
    }

    let committed = allocations(|| turn(&mut slice, paint, 30.0, false));
    assert_eq!(committed, 0);
}

/// Commits a scroll of `scroll` to `offset` pixels down.
fn scroll_to(slice: &mut Captured, scroll: ScrollId, offset: f32) {
    let mut tx = slice.tx();
    tx.scroll_to(scroll, [0.0, offset]);
    slice.compositor.commit(tx).expect("the scroll commits");
}

#[test]
fn a_warm_whole_pixel_scroll_that_reveals_no_new_row_builds_its_frame_without_allocating() {
    let mut slice = Slice::new(CaptureRenderer::with_capabilities(FULL));
    slice.compositor.set_verification(false);
    slice.draw();
    let scroll = list(&mut slice, 400, false).scroll;
    for offset in 1..5 {
        scroll_to(&mut slice, scroll, offset as f32);
        assert!(slice.draw());
    }

    scroll_to(&mut slice, scroll, 5.0);
    let built = allocations(|| assert_eq!(slice.build_only(), Built::Frame));
    assert_eq!(slice.frame.shifts.len(), 1);
    assert_eq!(built, 0);
}

#[test]
fn a_warm_commit_that_moves_a_list_row_to_its_end_allocates_nothing() {
    let mut slice = Slice::new(CaptureRenderer::with_capabilities(FULL));
    slice.compositor.set_verification(false);
    slice.draw();
    let list = list(&mut slice, 40, false);
    let move_first_row = |slice: &mut Captured, row: u32| {
        let mut tx = slice.tx();
        let key = ChunkKey::new(2, row);
        tx.splice(list.node, 0, 1, &[]);
        tx.splice(list.node, 39, 0, &[OrderEntry::Chunk(key)]);
        tx.set_offset(key, [0.0, 400.0]);
        slice.compositor.commit(tx).expect("the moved row commits");
    };
    for row in 0..8 {
        move_first_row(&mut slice, row);
        assert!(slice.draw());
    }

    let committed = allocations(|| move_first_row(&mut slice, 8));
    assert_eq!(committed, 0);
}

#[test]
fn a_warm_commit_that_moves_twelve_list_rows_by_their_offsets_allocates_nothing() {
    let mut slice = Slice::new(CaptureRenderer::with_capabilities(FULL));
    slice.compositor.set_verification(false);
    slice.draw();
    list(&mut slice, 40, false);
    let move_rows = |slice: &mut Captured, by: f32| {
        let mut tx = slice.tx();
        for row in 0..12 {
            tx.set_offset(ChunkKey::new(2, row), [0.0, by]);
        }
        slice.compositor.commit(tx).expect("the moved rows commit");
    };
    for turn in 0..4 {
        move_rows(&mut slice, (turn % 2) as f32);
        assert!(slice.draw());
    }

    let committed = allocations(|| move_rows(&mut slice, 0.0));
    assert_eq!(committed, 0);
}
