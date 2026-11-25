//! Frames built after a sequence of transactions draw what one transaction of the same final
//! state draws.

mod common;

use iceblink::RenderCapabilities;
use iceblink_testing::capture::CaptureRenderer;

use common::canon::canonical;
use common::slice::Slice;
use common::stories::ALL;
use common::story::Story;
use common::{FULL, stat};

/// Returns the canonical drawing of a whole-output frame after `story` played one step per
/// frame for a renderer with `caps`.
fn incremental(story: &Story, caps: RenderCapabilities) -> String {
    let mut slice = Slice::new(CaptureRenderer::with_capabilities(caps));
    slice.draw();
    slice.play(story);
    slice.redraw_all();
    assert_eq!(stat(&slice.compositor, "order_overlaps"), 0);
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
    canonical(&slice.frame, slice.renderer.mirror())
}

/// Returns the canonical drawing of the first frame after every step of `story` was committed
/// in one transaction.
fn full(story: &Story) -> String {
    let mut slice = Slice::new(CaptureRenderer::with_capabilities(FULL));
    slice.apply_all(story);
    assert!(slice.draw(), "the first frame is built");
    canonical(&slice.frame, slice.renderer.mirror())
}

#[test]
fn every_story_draws_the_same_incrementally_and_whole() {
    for story in &ALL {
        let incremental = incremental(story, FULL);
        let full = full(story);
        if let Some(difference) = iceblink_testing::golden::first_difference(&full, &incremental) {
            panic!("{} differs: {difference}", story.name);
        }
    }
}

#[test]
fn a_renderer_without_resident_chunks_draws_the_same_from_transient_copies() {
    for story in &ALL {
        let transient = incremental(story, RenderCapabilities::MINIMAL);
        let resident = incremental(story, FULL);
        if let Some(difference) = iceblink_testing::golden::first_difference(&resident, &transient)
        {
            panic!("{} differs: {difference}", story.name);
        }
    }
}
