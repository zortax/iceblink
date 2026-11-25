//! Frames of the naive compositor recorded by the capture renderer.

mod common;

use iceblink::{Built, Frame, FrameClock, FrameRequest, RenderTarget, SubpixelOrder};
use iceblink::{Compositor, Renderer};
use iceblink_testing::capture::CaptureRenderer;
use iceblink_testing::validate_frame;

use crate::common::naive::Harness;
use crate::common::naive::scene::{FULL, build, nudge};

/// Builds and records the next frame of `harness`'s output, and returns whether one was built.
fn record(harness: &mut Harness, renderer: &mut CaptureRenderer, frame: &mut Frame) -> bool {
    let caps = renderer.capabilities();
    let request = FrameRequest {
        caps: &caps,
        age: renderer.target_age(),
        clock: FrameClock::default(),
        subpixel: Some(SubpixelOrder::RedToBlue),
    };
    if harness
        .compositor
        .build_frame(harness.output, &request, frame)
        != Built::Frame
    {
        return false;
    }

    if let Err(violations) = validate_frame(frame) {
        panic!(
            "frame {} breaks the frame contract:\n{}",
            frame.id.0,
            iceblink_testing::validate::describe(&violations)
        );
    }
    let outcome = renderer.draw(frame);
    harness
        .compositor
        .frame_done(harness.output, frame.id, &outcome);
    true
}

/// A capture renderer with every capability, configured for the scene's output.
fn full_renderer() -> CaptureRenderer {
    let mut renderer = CaptureRenderer::with_capabilities(FULL);
    renderer.configure(RenderTarget {
        size: [96, 64],
        scale: 1.0,
        opaque: true,
    });
    renderer
}

/// Returns a transcript with the position of every tile on its page left out.
fn without_tiles(transcript: &str) -> String {
    let mut out = String::with_capacity(transcript.len());
    let mut rest = transcript;
    while let Some(at) = rest.find("tile=page") {
        let (head, tail) = rest.split_at(at);
        out.push_str(head);
        let end = tail.find(']').map_or(tail.len(), |end| end + 1);
        out.push_str("tile");
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

#[test]
fn an_edited_scene_draws_what_a_scene_built_from_scratch_draws() {
    let mut harness = Harness::with_size([96, 64]);
    let mut renderer = full_renderer();
    let mut frame = Frame::default();
    build(&mut harness);
    assert!(record(&mut harness, &mut renderer, &mut frame));

    nudge(&mut harness);
    assert!(record(&mut harness, &mut renderer, &mut frame));
    let edited = renderer
        .last()
        .expect("a frame")
        .drawing
        .as_str()
        .to_owned();

    let mut fresh = Harness::with_size([96, 64]);
    let mut fresh_renderer = full_renderer();
    let mut fresh_frame = Frame::default();
    build(&mut fresh);
    nudge(&mut fresh);
    assert!(record(&mut fresh, &mut fresh_renderer, &mut fresh_frame));
    let scratch = fresh_renderer
        .last()
        .expect("a frame")
        .drawing
        .as_str()
        .to_owned();

    // The moved run's glyphs take new phases, which the atlas places beside the old tiles.
    assert_eq!(without_tiles(&edited), without_tiles(&scratch));
}
