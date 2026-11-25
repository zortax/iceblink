//! Animations on a compositor without the animation tier.

use iceblink::smallvec::smallvec;
use iceblink::{
    Animation, AnimationStart, AnimationTarget, AnyId, Direction, Easing, FillMode, Keyframe,
    Keyframes, Matrix4, RejectReason, Timing,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::identity;
use crate::conformance::scene::cases::rejections::rejects;
use crate::conformance::scene::stage::Stage;

/// A compositor without the animation tier rejects animations, checking nothing else about them,
/// and commits a cancel or a removal of an animation id.
pub fn unsupported_animations(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    if stage.compositor.capabilities().animations {
        return;
    }
    let mut tx = stage.tx();
    tx.cancel_animation(stage.ids.mint());
    tx.remove(AnyId::Animation(stage.ids.mint()));
    stage.commit(tx);

    let mut tx = stage.tx();
    let moved = tx.create_transform(identity(None));
    tx.remove(moved);
    tx.animate(Animation {
        target: AnimationTarget::Transform(moved),
        keyframes: Keyframes::Matrix(smallvec![
            Keyframe {
                offset: 0.0,
                value: Matrix4::IDENTITY,
                easing: Easing::Linear,
            },
            Keyframe {
                offset: 1.0,
                value: Matrix4::translation(10.0, 0.0, 0.0),
                easing: Easing::Linear,
            },
        ]),
        timing: Timing {
            duration_ns: 1_000_000,
            delay_ns: 0,
            iterations: 1.0,
            direction: Direction::Normal,
            fill: FillMode::None,
            easing: Easing::Linear,
        },
        start: AnimationStart::Next,
    });
    rejects(
        &mut stage,
        tx,
        RejectReason::Unsupported,
        "an animation of a removed transform",
    );
}
