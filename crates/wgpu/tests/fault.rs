//! Every answer an acquisition can give, and what the next frame shows.
//!
//! A frame's damage retires once its work is submitted, whether or not it was presented, so after
//! any run of failed acquisitions the next presented image holds the whole picture.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{SIDE, device_lock, offscreen};
use common::present::Rig;
use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{Frame, FrameOutcome, PaintRef, SkipReason};
use iceblink_wgpu::{Acquisition, Builder, wgpu};

/// The white background and the red square every frame here paints.
#[derive(Clone, Copy)]
struct Paints {
    white: PaintRef,
    red: PaintRef,
}

/// Returns a first frame with the square at `step`, drawn whole.
fn first(step: i32) -> (Paints, Frame) {
    let mut builder = FrameBuilder::new();
    let paints = Paints {
        white: builder.solid(opaque(255, 255, 255)),
        red: builder.solid(opaque(220, 30, 30)),
    };
    square(&mut builder, paints, step);
    (paints, builder.finish())
}

/// Returns a later frame that moves the square from `before` to `after`.
fn moved(paints: Paints, before: i32, after: i32) -> Frame {
    let mut damage = DamageSet::new();
    for step in [before, after] {
        damage.absorb(Rect::new(Point::new(7 + step * 8, 39), Size::new(34, 34)));
    }
    let mut builder = FrameBuilder::next().damage(damage);
    square(&mut builder, paints, after);
    builder.finish()
}

/// Adds the white background and the red square at `step`.
fn square(builder: &mut FrameBuilder, paints: Paints, step: i32) {
    builder.quad(filled(
        rect(0.0, 0.0, SIDE as f32, SIDE as f32),
        paints.white,
    ));
    builder.quad(filled(
        rect(8.0 + step as f32 * 8.0, 40.0, 32.0, 32.0),
        paints.red,
    ));
}

fn renderer() -> iceblink_wgpu::WgpuRenderer {
    offscreen(wgpu::TextureFormat::Bgra8Unorm)
}

/// Returns a rig that holds the device lock and a clean rig beside it.
fn pair() -> (Rig, Rig) {
    let guard = device_lock();
    (Rig::new(renderer(), guard), Rig::beside(renderer()))
}

#[test]
fn no_answer_an_acquisition_can_give_loses_a_frame() {
    let (mut faulted, mut clean) = pair();
    for injected in Acquisition::ALL {
        let (paints, start) = first(0);
        faulted.present(&start);

        faulted.offscreen().inject_fault(injected, 1);
        let outcome = faulted.draw(&moved(paints, 0, 1));
        assert!(
            outcome.retires_damage(),
            "{injected:?} lost its frame's damage"
        );

        let after = faulted.draw(&moved(paints, 1, 2));
        assert!(after.stats().is_some(), "after {injected:?}: {after:?}");
        let recovered = faulted
            .read_presented()
            .expect("an offscreen image is read back");
        let reference = clean.present(&first(2).1);
        assert_eq!(
            recovered.max_difference(&reference),
            0,
            "after {injected:?} the next image is not the whole picture"
        );
    }
}

#[test]
fn an_answer_without_an_image_leaves_the_previous_frame_presented() {
    let guard = device_lock();
    let mut rig = Rig::new(renderer(), guard);
    let (paints, start) = first(0);
    let shown = rig.present(&start);

    rig.offscreen().inject_fault(Acquisition::Timeout, 1);
    let outcome = rig.draw(&moved(paints, 0, 1));
    assert_eq!(outcome, FrameOutcome::Skipped(SkipReason::Timeout));
    let presented = rig
        .read_presented()
        .expect("an offscreen image is read back");
    assert_eq!(presented.max_difference(&shown), 0, "nothing was copied");
    assert!(
        rig.read_composed().max_difference(&shown) > 0,
        "the kept target moved on, which is why the damage retires"
    );
}

#[test]
fn a_run_of_occluded_answers_ends_in_the_whole_picture() {
    let (mut faulted, mut clean) = pair();
    let (paints, start) = first(0);
    faulted.present(&start);

    faulted.offscreen().inject_fault(Acquisition::Occluded, 3);
    for step in 1..4 {
        let outcome = faulted.draw(&moved(paints, step - 1, step));
        assert_eq!(outcome, FrameOutcome::Skipped(SkipReason::Occluded));
        assert!(outcome.retires_damage());
        assert!(!outcome.wants_another_frame(), "an occluded output parks");
    }

    let outcome = faulted.draw(&moved(paints, 3, 4));
    assert!(
        outcome.stats().is_some(),
        "the injection ran out: {outcome:?}"
    );
    let recovered = faulted
        .read_presented()
        .expect("an offscreen image is read back");
    assert_eq!(recovered.max_difference(&clean.present(&first(4).1)), 0);
}

#[test]
fn only_a_frame_that_is_presented_is_announced() {
    let guard = device_lock();
    let notified = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&notified);
    let renderer = Builder::new()
        .with_pre_present(Box::new(move |_| {
            counter.fetch_add(1, Ordering::Relaxed);
        }))
        .offscreen([SIDE, SIDE], wgpu::TextureFormat::Bgra8Unorm)
        .unwrap_or_else(|failure| panic!("no usable graphics device: {failure}"));
    let mut rig = Rig::new(renderer, guard);
    assert_eq!(notified.load(Ordering::Relaxed), 0);

    let (paints, start) = first(0);
    rig.offscreen().inject_fault(Acquisition::Timeout, 1);
    rig.draw(&start);
    for (step, answer) in [
        Acquisition::Occluded,
        Acquisition::Outdated,
        Acquisition::Lost,
        Acquisition::Validation,
    ]
    .into_iter()
    .enumerate()
    {
        let step = step as i32;
        rig.offscreen().inject_fault(answer, 1);
        rig.draw(&moved(paints, step, step + 1));
        assert_eq!(
            notified.load(Ordering::Relaxed),
            0,
            "a {answer:?} acquisition announced a frame that is not presented"
        );
    }

    let quiet = FrameBuilder::next().damage(DamageSet::new());
    assert!(rig.draw(&quiet.finish()).stats().is_some());
    assert_eq!(
        notified.load(Ordering::Relaxed),
        1,
        "the composed frame was presented and announced after the failures"
    );

    rig.offscreen().inject_fault(Acquisition::Suboptimal, 1);
    assert!(rig.draw(&moved(paints, 4, 5)).stats().is_some());
    assert_eq!(
        notified.load(Ordering::Relaxed),
        2,
        "a suboptimal image is presented and announced"
    );
}
