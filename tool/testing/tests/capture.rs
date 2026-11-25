//! The capture renderer: outcomes, the kept target, recorded frames and the mirror.

mod common;

use iceblink::Renderer;
use iceblink::{BufferAge, FrameOutcome, PageFormat, RenderTarget, SkipReason};
use iceblink_testing::capture::CaptureRenderer;

use crate::common::capture::{SIDE, kitchen_sink};

/// A square opaque target of `side` pixels.
fn target(side: u32) -> RenderTarget {
    RenderTarget {
        size: [side, side],
        scale: 1.0,
        opaque: true,
    }
}

#[test]
fn a_frame_before_configure_is_unconfigured_and_still_applied() {
    let mut renderer = CaptureRenderer::new();
    let outcome = renderer.draw(&kitchen_sink());
    assert_eq!(outcome, FrameOutcome::Skipped(SkipReason::Unconfigured));
    assert_eq!(renderer.target_age(), BufferAge::Unknown);
    assert_eq!(renderer.mirror().pages.len(), 2);
    assert_eq!(renderer.frames(), 1);
}

#[test]
fn a_presented_frame_reports_its_plan_and_keeps_the_target() {
    let mut renderer = CaptureRenderer::new();
    renderer.configure(target(SIDE));
    let stats = renderer
        .draw(&kitchen_sink())
        .stats()
        .expect("a configured capture presents");
    assert_eq!(stats.vector_passes, 1);
    assert_eq!(stats.chunks_resident, 1);
    assert_eq!(stats.draw_calls, 0);
    assert_eq!(renderer.target_age(), BufferAge::Kept);

    renderer.configure(target(SIDE * 2));
    assert_eq!(renderer.target_age(), BufferAge::Unknown);
}

#[test]
fn the_last_frame_is_kept_unless_every_frame_is() {
    let mut last = CaptureRenderer::new();
    let mut all = CaptureRenderer::new().keeping_all();
    for _ in 0..3 {
        last.draw(&kitchen_sink());
        all.draw(&kitchen_sink());
    }
    assert_eq!(last.captured().len(), 1);
    assert_eq!(all.captured().len(), 3);
    assert_eq!(all.take().len(), 3);
    assert!(all.captured().is_empty());
    let kept = last.last().expect("a frame was drawn");
    assert_eq!(kept.frame.lanes.quads.sources.len(), 3);
    assert!(kept.transcript.as_str().starts_with("frame 1\n"));
}

#[test]
fn uploads_reach_the_mirrored_page_and_a_lost_device_forgets_them() {
    let mut renderer = CaptureRenderer::new();
    renderer.draw(&kitchen_sink());
    let page = &renderer.mirror().pages[&0];
    assert_eq!(page.format, PageFormat::A8);
    assert_eq!(page.texel(3, 3), Some(&[0xff][..]));
    assert_eq!(page.texel(4, 4), Some(&[0][..]));

    renderer.lose_device();
    assert!(renderer.mirror().pages.is_empty());
    assert!(renderer.mirror().resident.is_empty());
}
