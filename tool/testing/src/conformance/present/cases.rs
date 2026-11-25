//! The cases of the present suite.

use std::collections::BTreeMap;
use std::time::Instant;

use iceblink::damage::DamageSet;
use iceblink::{Acquired, BufferAge, Color, FrameOutcome, FramePaint, PresentMode, SkipReason};

use crate::conformance::draw::builder::{FrameBuilder, area, filled};
use crate::conformance::draw::same_image;
use crate::conformance::present::MakeRig;
use crate::conformance::present::stand::{SIDE, Stand, centre, filled_frame, skip};

const RED: [u8; 3] = [220, 20, 20];

/// A frame drawn before the renderer is configured records nothing and keeps its damage.
pub fn unconfigured(make: MakeRig<'_>) {
    let Some(mut stand) = Stand::unconfigured(make, PresentMode::KeptTarget, "unconfigured") else {
        return;
    };
    let outcome = stand.draw(filled_frame(RED).0);
    assert_eq!(
        outcome,
        FrameOutcome::Skipped(SkipReason::Unconfigured),
        "a frame without a target is skipped as unconfigured"
    );
    assert!(
        !outcome.retires_damage(),
        "an unconfigured frame keeps its damage"
    );

    stand.configure();
    let outcome = stand.draw(filled_frame(RED).0);
    assert!(
        matches!(outcome, FrameOutcome::Presented(_)),
        "once configured the frame is presented: {outcome:?}"
    );
    assert_eq!(centre(&stand.presented())[..3], RED);
}

/// A kept target's age is `Unknown` until a frame is composed into it, and `Kept` after.
pub fn kept_target_age(make: MakeRig<'_>) {
    let Some(mut stand) = Stand::open(make, PresentMode::KeptTarget, "kept_target_age") else {
        return;
    };
    assert_eq!(
        stand.rig.renderer().target_age(),
        BufferAge::Unknown,
        "a new target holds nothing"
    );
    let outcome = stand.draw(filled_frame(RED).0);
    assert!(matches!(outcome, FrameOutcome::Presented(_)), "{outcome:?}");
    assert_eq!(
        stand.rig.renderer().target_age(),
        BufferAge::Kept,
        "a target holds the frame composed into it"
    );
}

/// A frame without damage is skipped as undamaged once the last composed frame was presented,
/// and still applies its table deltas.
pub fn undamaged_frames(make: MakeRig<'_>) {
    let Some(mut stand) = Stand::open(make, PresentMode::KeptTarget, "undamaged_frames") else {
        return;
    };
    let (first, paint) = filled_frame(RED);
    assert!(matches!(stand.draw(first), FrameOutcome::Presented(_)));

    let mut quiet = FrameBuilder::next([SIDE, SIDE]).damage(DamageSet::new());
    quiet.set_paint(paint.index, FramePaint::Solid(Color([0.0, 0.0, 1.0, 1.0])));
    let outcome = stand.draw(quiet.finish());
    assert_eq!(
        outcome,
        FrameOutcome::Skipped(SkipReason::Undamaged),
        "a frame without damage after a presented frame is skipped"
    );
    assert!(!outcome.wants_another_frame());

    let mut whole = FrameBuilder::next([SIDE, SIDE]).damage(DamageSet::full());
    whole.quad(filled([0.0, 0.0, SIDE as f32, SIDE as f32], paint));
    assert!(matches!(
        stand.draw(whole.finish()),
        FrameOutcome::Presented(_)
    ));
    assert_eq!(
        centre(&stand.presented()),
        [0, 0, 255, 255],
        "the skipped frame's paint write was applied"
    );
}

/// A frame whose present waits for the platform is held, retires its damage, and is presented
/// by `present_held` without drawing again once the platform takes it.
pub fn held_frames(make: MakeRig<'_>) {
    let Some(mut stand) = Stand::open(make, PresentMode::KeptTarget, "held_frames") else {
        return;
    };
    if !stand.rig.hold_presents(true) {
        skip("held_frames", "the rig cannot hold presents");
        return;
    }
    let outcome = stand.draw(filled_frame(RED).0);
    assert_eq!(outcome, FrameOutcome::Skipped(SkipReason::Held));
    assert!(outcome.retires_damage() && !outcome.wants_another_frame());
    assert_eq!(
        stand.rig.renderer().present_held(),
        FrameOutcome::Skipped(SkipReason::Held),
        "a held frame stays held while the platform takes none"
    );

    stand.rig.hold_presents(false);
    let outcome = stand.rig.renderer().present_held();
    assert!(
        matches!(outcome, FrameOutcome::Presented(_)),
        "present_held presents the held frame: {outcome:?}"
    );
    assert_eq!(centre(&stand.presented())[..3], RED);
    assert_eq!(
        stand.rig.renderer().present_held(),
        FrameOutcome::Skipped(SkipReason::Undamaged),
        "nothing is held after the held frame was presented"
    );
}

/// A frame composed while acquisition timed out retires its damage, and the next frame presents
/// it even without damage of its own.
pub fn timed_out_acquisitions(make: MakeRig<'_>) {
    let Some(mut stand) = Stand::open(make, PresentMode::KeptTarget, "timed_out_acquisitions")
    else {
        return;
    };
    if !stand.rig.time_out_acquisitions(1) {
        skip(
            "timed_out_acquisitions",
            "the rig cannot time out acquisitions",
        );
        return;
    }
    let outcome = stand.draw(filled_frame(RED).0);
    assert_eq!(outcome, FrameOutcome::Skipped(SkipReason::Timeout));
    assert!(outcome.retires_damage() && outcome.wants_another_frame());

    let quiet = FrameBuilder::next([SIDE, SIDE]).damage(DamageSet::new());
    let outcome = stand.draw(quiet.finish());
    assert!(
        matches!(outcome, FrameOutcome::Presented(_)),
        "the next frame presents the composed one: {outcome:?}"
    );
    assert_eq!(centre(&stand.presented())[..3], RED);
}

/// After a device loss a renderer either recovers, and then draws a full frame as before, or
/// reports the device unavailable and keeps the damage.
pub fn device_loss(make: MakeRig<'_>) {
    let Some(mut stand) = Stand::open(make, PresentMode::KeptTarget, "device_loss") else {
        return;
    };
    let (first, paint) = filled_frame(RED);
    assert!(matches!(stand.draw(first), FrameOutcome::Presented(_)));
    let before = stand.presented();
    if !stand.rig.lose_device() {
        skip("device_loss", "the rig cannot lose its device");
        return;
    }

    let mut next = FrameBuilder::next([SIDE, SIDE]).damage(area(0, 0, 8, 8));
    next.quad(filled([0.0, 0.0, 8.0, 8.0], paint));
    match stand.draw(next.finish()) {
        outcome @ FrameOutcome::Recovered => {
            assert!(outcome.retires_damage() && outcome.wants_another_frame());

            let (whole, _) = filled_frame(RED);
            assert!(matches!(stand.draw(whole), FrameOutcome::Presented(_)));
            same_image(
                &before,
                &stand.presented(),
                0,
                "the first frame after a recovery",
            );
        }
        outcome @ FrameOutcome::Skipped(SkipReason::DeviceUnavailable) => {
            assert!(!outcome.retires_damage() && !outcome.wants_another_frame());
        }
        other => panic!("a frame after a device loss is recovered or unavailable, not {other:?}"),
    }
}

/// In buffer-age mode, frames compose into the acquired image, and an image of age `n` is the
/// one presented `n` frames before.
pub fn buffer_age(make: MakeRig<'_>) {
    let Some(mut stand) = Stand::open(make, PresentMode::BufferAge, "buffer_age") else {
        return;
    };
    assert_eq!(
        stand.rig.renderer().target_age(),
        BufferAge::Unknown,
        "a renderer in buffer-age mode reports no kept target"
    );
    let mut presented_at: BTreeMap<u32, usize> = BTreeMap::new();
    for frame in 0..6usize {
        let presenter = stand
            .rig
            .renderer()
            .presenter()
            .expect("a renderer in buffer-age mode has a presenter");
        let Acquired::Image { image, age } = presenter.acquire() else {
            panic!("an image is acquired");
        };
        if let BufferAge::Known(n) = age {
            assert!(n >= 1, "an age is at least one");
            assert_eq!(
                presented_at.get(&image).map(|at| frame - at),
                Some(n as usize),
                "image {image} of age {n} was presented {n} frames before"
            );
        }
        let colour = [40 * frame as u8, 255 - 40 * frame as u8, 90];
        let outcome = stand.draw(filled_frame(colour).0);
        assert!(matches!(outcome, FrameOutcome::Presented(_)), "{outcome:?}");
        assert_eq!(centre(&stand.presented())[..3], colour);
        presented_at.insert(image, frame);
    }
}

/// The time the last frame waited for an image is part of the time its draw took.
pub fn acquire_block(make: MakeRig<'_>) {
    let Some(mut stand) = Stand::open(make, PresentMode::KeptTarget, "acquire_block") else {
        return;
    };
    for _ in 0..3 {
        let started = Instant::now();
        let outcome = stand.draw(filled_frame(RED).0);
        let took = started.elapsed();
        assert!(matches!(outcome, FrameOutcome::Presented(_)), "{outcome:?}");
        let waited = stand.rig.renderer().acquire_block();
        assert!(
            waited <= took,
            "the frame waited {waited:?} for an image but its draw took {took:?}"
        );
    }
}
