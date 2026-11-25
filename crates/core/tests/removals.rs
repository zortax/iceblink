//! The reference check of removed objects, which runs only in commits that remove one.

mod common;

use iceblink::Compositor;
use iceblink::{Color, Paint};
use iceblink_testing::capture::CaptureRenderer;

use common::slice::Slice;
use common::{FULL, stat};

#[test]
fn only_a_commit_that_removes_an_object_counts_references() {
    let mut slice = Slice::new(CaptureRenderer::with_capabilities(FULL));
    let mut tx = slice.tx();
    let paint = slice.world.solid(&mut tx, [0.2, 0.3, 0.4, 1.0]);
    slice.compositor.commit(tx).expect("the paint commits");
    let before = stat(&slice.compositor, "work.use_checks");

    let mut tx = slice.tx();
    tx.set_paint(paint, Paint::Solid(Color([0.5, 0.3, 0.4, 1.0])));
    slice.compositor.commit(tx).expect("the write commits");
    assert_eq!(stat(&slice.compositor, "work.use_checks"), before);

    let mut tx = slice.tx();
    tx.remove(paint);
    slice.compositor.commit(tx).expect("the removal commits");
    assert_eq!(stat(&slice.compositor, "work.use_checks"), before + 1);
}
