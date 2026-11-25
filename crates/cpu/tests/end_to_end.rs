//! The end-to-end conformance suite on the CPU renderer, and the pairs of compositor and renderer
//! that compare the CPU renderer, iceblink-wgpu, iceblink-core and the naive compositor of
//! iceblink-testing.

mod common;

use iceblink::Compositor;
use iceblink_core::CompositorImpl;
use iceblink_testing::conformance::end_to_end::{
    Pair, adopted_motion_equals_whole, driven_motion_equals_whole, incremental_equals_whole,
    pairs_agree,
};
use iceblink_testing::golden::Tolerance;
use iceblink_testing::gpu::device_lock;
use iceblink_testing::naive::NaiveCompositor;

use crate::common::targets::{on_cpu, on_gpu};

fn core() -> Box<dyn Compositor> {
    Box::new(CompositorImpl::new())
}

fn naive() -> Box<dyn Compositor> {
    Box::new(NaiveCompositor::new())
}

/// How far iceblink-wgpu and the CPU renderer may differ on one compositor's frames: four
/// steps.
// Measured over the eight scenes: at most 5 pixels a frame beyond 2 steps on NVIDIA and 2 on
// lavapipe, by up to 3. They lie on a translucent ellipse's edge, inside a quad blurred by 1.5
// in a group at opacity 0.9, and under a drop shadow. Four steps leaves a step of margin.
const RENDERERS: Tolerance = Tolerance {
    max_channel: 4,
    max_pixels: 0,
};

/// How far iceblink-core and the naive compositor may differ on one renderer: three steps.
// Measured over the eight scenes: no pixel differs on NVIDIA, and at most 33 pixels a frame by up
// to 2 on lavapipe.
const COMPOSITORS: Tolerance = Tolerance {
    max_channel: 3,
    max_pixels: 0,
};

/// How far naive with the CPU renderer and core with iceblink-wgpu may differ: four steps.
// Measured over the eight scenes: at most 5 pixels a frame beyond 2 steps on NVIDIA and 1 on
// lavapipe, by up to 3.
const BOTH: Tolerance = Tolerance {
    max_channel: 4,
    max_pixels: 0,
};

fn pair<'a>(
    compositor: &'a dyn Fn() -> Box<dyn Compositor>,
    target: iceblink_testing::conformance::draw::MakeTarget<'a>,
) -> Pair<'a> {
    Pair {
        compositor,
        target,
        caps: None,
    }
}

#[test]
fn core_on_the_cpu_draws_incremental_frames_as_whole_ones() {
    incremental_equals_whole(&core, &on_cpu);
}

#[test]
fn core_on_the_cpu_draws_compositor_scrolling_as_whole_frames() {
    driven_motion_equals_whole(&core, &on_cpu);
}

#[test]
fn core_on_the_cpu_draws_adopted_scrolling_as_whole_frames() {
    adopted_motion_equals_whole(&core, &on_cpu);
}

#[test]
fn core_on_the_gpu_and_naive_on_the_gpu_agree() {
    let _device = device_lock();
    pairs_agree(pair(&core, &on_gpu), pair(&naive, &on_gpu), COMPOSITORS);
}

#[test]
fn core_on_the_gpu_and_core_on_the_cpu_agree() {
    let _device = device_lock();
    pairs_agree(pair(&core, &on_gpu), pair(&core, &on_cpu), RENDERERS);
}

#[test]
fn naive_on_the_cpu_and_core_on_the_gpu_agree() {
    let _device = device_lock();
    pairs_agree(pair(&naive, &on_cpu), pair(&core, &on_gpu), BOTH);
}
