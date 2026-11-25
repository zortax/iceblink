//! GPU pass timing on a device with timestamp queries.

mod common;

use std::sync::Arc;

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{SIDE, plain_renderer, present};
use iceblink::{BlendMode, FRAME_NO_CLIP, FrameGroup};
use iceblink_wgpu::{GpuTiming, wgpu};

/// Returns a frame of a quad and a half-transparent group, so the frame has draws, a group and a
/// copy.
fn frame() -> iceblink::Frame {
    let mut builder = FrameBuilder::new();
    let white = builder.solid(opaque(255, 255, 255));
    let black = builder.solid(opaque(0, 0, 0));
    builder.quad(filled(rect(0.0, 0.0, SIDE as f32, SIDE as f32), white));
    builder.begin_group(FrameGroup {
        bounds: rect(16.0, 16.0, 96.0, 96.0),
        source: rect(16.0, 16.0, 96.0, 96.0),
        clip: FRAME_NO_CLIP,
        opacity: 0.5,
        blend: BlendMode::NORMAL,
        filters: Arc::from(Vec::new()),
    });
    builder.quad(filled(rect(16.0, 16.0, 64.0, 64.0), black));
    builder.end_group();
    builder.finish()
}

/// Asserts that every time of `timing` is finite and not negative, and that nothing went untimed.
fn assert_sane(timing: &GpuTiming) {
    for (name, ms) in [
        ("draws", timing.draws_ms),
        ("groups", timing.groups_ms),
        ("vectors", timing.vectors_ms),
        ("blit", timing.blit_ms),
        ("total", timing.total_ms()),
    ] {
        assert!(
            ms.is_finite() && ms >= 0.0,
            "{name} is {ms} ms in {timing:?}"
        );
    }
    assert_eq!(timing.untimed, 0, "a small frame fits the query set");
}

// Timing is asked for before the device opens and holds for the whole process, so this test keeps
// a binary of its own.
#[test]
fn a_timed_frame_reports_non_negative_gpu_time_per_kind() {
    iceblink_wgpu::enable_gpu_timing();
    let mut renderer = plain_renderer();
    let timestamps = renderer
        .gpu()
        .adapter()
        .features()
        .contains(wgpu::Features::TIMESTAMP_QUERY);

    assert_eq!(
        renderer.gpu_timing_enabled(),
        timestamps,
        "a device times frames exactly when its adapter has timestamp queries"
    );
    assert!(
        timestamps,
        "{} has no timestamp queries",
        renderer.gpu().describe()
    );
    assert_eq!(renderer.gpu_timed_frames(), Some(0));
    assert_eq!(renderer.gpu_timing(), None);

    present(&mut renderer, &frame());
    present(&mut renderer, &frame());
    assert_eq!(renderer.gpu_timed_frames(), Some(2));

    renderer.gpu().wait();
    let timings = renderer.take_gpu_timings();
    assert_eq!(
        timings
            .iter()
            .map(|timing| timing.frame)
            .collect::<Vec<_>>(),
        [0, 1],
        "both frames were timed and both results arrived after the wait: {timings:?}"
    );
    for timing in &timings {
        assert_sane(timing);
    }
    assert_eq!(renderer.gpu_timing(), timings.last().copied());
    assert!(
        renderer.take_gpu_timings().is_empty(),
        "a result is handed over once"
    );
}
