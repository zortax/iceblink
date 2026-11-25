//! A scene with every item kind, node kind and clip kind, edited over several frames, whose every
//! frame passes the frame contract.

mod common;

use iceblink::Compositor;
use iceblink::{BatchKind, FrameOutcome, RenderCapabilities, SubpixelOrder};

use crate::common::naive::Harness;
use crate::common::naive::scene::{FULL, build, nudge};

/// Builds the scene for a renderer of `caps`, then draws it moved, after a recovery, and with
/// every cache dropped.
fn run_scene(caps: RenderCapabilities, subpixel: Option<SubpixelOrder>) -> Harness {
    let mut harness = Harness::with_size([96, 64]);
    harness.set_caps(caps);
    harness.set_subpixel(subpixel);
    build(&mut harness);
    assert!(harness.frame());

    nudge(&mut harness);
    assert!(harness.frame_with(FrameOutcome::Recovered));
    assert!(harness.frame());

    harness.compositor.trim(0);
    harness.compositor.forget_caches();
    assert!(harness.frame());
    harness
}

#[test]
fn every_kind_lowers_to_a_valid_frame_for_a_minimal_renderer() {
    let harness = run_scene(RenderCapabilities::MINIMAL, None);
    let frame = &harness.frame;
    assert!(frame.lanes.shader_quads.items.is_empty());
    assert!(frame.resources.shaders.is_empty());
    assert!(frame.lanes.subpixel.items.is_empty());
}

#[test]
fn every_kind_lowers_to_a_valid_frame_for_a_full_renderer() {
    let harness = run_scene(FULL, Some(SubpixelOrder::RedToBlue));
    let frame = &harness.frame;
    assert_eq!(frame.lanes.shader_quads.items.len(), 2);
    assert!(!frame.lanes.subpixel.items.is_empty());
    assert!(!frame.lanes.color.items.is_empty());
    assert!(!frame.lanes.marks.items.is_empty());
    assert_eq!(frame.externals.len(), 1);
    assert!(
        frame
            .batches
            .iter()
            .any(|batch| matches!(batch.kind, BatchKind::Backdrop(_)))
    );
}
