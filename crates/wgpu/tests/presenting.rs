//! A frame held at the present gate and made obsolete by a configure.

mod common;

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{draw, plain_renderer};
use iceblink::Renderer as _;
use iceblink::{FrameOutcome, SkipReason};

#[test]
fn a_held_frame_is_not_presented_after_a_configure_for_another_target() {
    let mut renderer = plain_renderer();
    renderer.set_present_gate(Box::new(|| false));
    let mut frame = FrameBuilder::new();
    let red = frame.solid(opaque(255, 0, 0));
    frame.quad(filled(rect(0.0, 0.0, 64.0, 64.0), red));

    assert_eq!(
        draw(&mut renderer, &frame.finish()),
        FrameOutcome::Skipped(SkipReason::Held)
    );

    renderer.configure(iceblink::RenderTarget {
        size: [600, 600],
        scale: 1.0,
        opaque: true,
    });

    assert_eq!(
        renderer.present_held(),
        FrameOutcome::Skipped(SkipReason::Undamaged)
    );
}
