//! The conformance suites, which any implementation of a seam runs through a factory.
//!
//! Each suite is a set of public functions, one per case, that take a factory and panic with a
//! description of the first rule a case finds broken. An implementation declares one test per
//! case with the suite's macro, or runs every case in one test with the suite's `run_all`. A case
//! that needs a feature the implementation reports it lacks, or a hook its factory cannot
//! provide, passes after a `skipped:` line on standard error.
//!
//! | Suite | Factory | Proves |
//! | --- | --- | --- |
//! | [`scene`] | [`MakeCompositor`](scene::MakeCompositor): a fresh `Compositor` | Commit validation and every rejection, damage soundness for each kind of change, paint order, groups and the clips of effects, identical replace and precise damage where the compositor reports `precise`, and the frame contract on every frame |
//! | [`tier`] | [`Factory`](tier::Factory): a fresh `Compositor` | Animations, scroll input and hit testing, each case on a compositor that reports the features it needs |
//! | [`draw`] | [`MakeTarget`](draw::MakeTarget): a configured `Renderer` and a readback of its presented image | Every frame kind, paint, clip, group, filter, backdrop and shader against golden images, blend modes against their formulas, and shifts, partial redraws, resident chunks and opaque covers against the same picture drawn without them |
//! | [`present`] | [`MakeRig`](present::MakeRig): an unconfigured `Renderer` in a presenter mode, and hooks that provoke platform conditions | `Unconfigured`, `Undamaged`, `Held` with `present_held`, `Timeout`, `Recovered` or `DeviceUnavailable`, `acquire_block`, kept-target ages and buffer-age rotation |
//! | [`end_to_end`] | A compositor factory and a target factory | Random scenes and edits drawn incrementally equal the whole redraw, and two compositor and renderer pairs draw one transcript alike |
//! | [`client`] | [`Rig`](client::Rig): a compositor factory and a stepped driver around it | Commit answers, the step latch, gestures routed otherwise than predicted, scroll watches and wakes, unregistering, snapshots and stepped reproduction |
//!
//! # Plugging in an implementation
//!
//! A compositor runs the scene and tier suites with a closure that builds it:
//!
//! ```text
//! iceblink_testing::scene_suite!(|| -> Box<dyn iceblink::Compositor> {
//!     Box::new(my_compositor::Compositor::new())
//! });
//! iceblink_testing::tier_suite!(|| -> Box<dyn iceblink::Compositor> {
//!     Box::new(my_compositor::Compositor::new())
//! });
//! ```
//!
//! A renderer implements [`DrawTarget`](draw::DrawTarget) for itself and its readback, and
//! [`PresentRig`](present::PresentRig) for itself and its presenter, and passes functions that
//! build them to [`draw_suite!`](crate::draw_suite) and [`present_suite!`](crate::present_suite).
//! A factory returns `None` when it cannot build its target, after calling
//! [`gpu::skip`](crate::gpu::skip), which fails the test when `ICEBLINK_REQUIRE_GPU` is set. A
//! draw factory that shares one device between tests takes
//! [`gpu::device_lock`](crate::gpu::device_lock) and holds it in the target, and an end-to-end test
//! takes it once for both of its targets. The end-to-end functions are called from ordinary tests,
//! with a compositor factory and a target factory for each pair.
//!
//! # Tolerances
//!
//! The draw goldens are iceblink-wgpu's images on NVIDIA hardware, with vello rasterising
//! paths. Each draw case declares a [`Tolerance`](crate::golden::Tolerance): the largest channel
//! difference a pixel may have, and how many pixels may differ by more. Analytic edges, corners,
//! lines and marks allow 3 steps in up to 24 to 48 pixels, gradients 2 steps everywhere, sampled
//! and composited texels 3 steps in up to 32 pixels, shadows 4 steps in up to 96 pixels, filters
//! and backdrops 4 steps in up to 128 pixels, blurs checked against the Gaussian 2 steps
//! everywhere, and rasterised path fills 6 steps in up to 96 pixels. Stroke edges allow 51 steps,
//! and at most 128 pixels may differ by more than 2 steps. Blend modes allow 3 steps from the
//! formula, and the sequences of frames compare with the same picture drawn whole on the same
//! renderer, within one step. The end-to-end suite allows 2 steps between an incremental frame and
//! its whole redraw, and takes the tolerance between two pairs as an argument.
//!
//! Goldens are rewritten by running the draw suite with `ICEBLINK_BLESS=1` on the reference
//! renderer.

pub mod client;
pub mod draw;
pub mod end_to_end;
pub mod present;
pub mod scene;
pub mod tier;

use std::panic::{AssertUnwindSafe, catch_unwind};

/// One case of a suite: a name and the function that runs it.
#[derive(Clone, Copy, Debug)]
pub struct Case<F> {
    /// The name, which is also the function's name.
    pub name: &'static str,
    /// Runs the case, panicking with a description of the first rule broken.
    pub run: F,
}

/// Runs every case of `cases` through `run`, and then fails naming each case that failed.
#[track_caller]
pub(crate) fn run_cases<F>(cases: &[Case<F>], mut run: impl FnMut(&Case<F>)) {
    let mut failed = Vec::new();

    for case in cases {
        eprintln!("case {}", case.name);
        if catch_unwind(AssertUnwindSafe(|| run(case))).is_err() {
            failed.push(case.name);
        }
    }
    assert!(failed.is_empty(), "failed cases: {}", failed.join(", "));
}
