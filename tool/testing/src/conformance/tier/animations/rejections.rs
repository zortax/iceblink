//! Rules of rejecting malformed animations.

use iceblink::{
    Animation, AnimationTarget, FillMode, Filter, FilterParam, Keyframes, RejectReason,
};

use crate::conformance::tier::animations::{effect_node, fade, faded, timing};
use crate::conformance::tier::stage::{Factory, Stage};

/// A commit with a malformed animation is rejected: an animation of a removed target with
/// `StaleId`, and keyframes that break the rules or a filter parameter the filter does not have
/// with `InvalidItem`.
pub fn malformed_animations_are_rejected(factory: Factory<'_>) {
    let (mut stage, effect) = faded(factory, 0.5, &[Filter::Opacity(1.0)]);
    let reject = |stage: &mut Stage, animation: Animation| {
        let mut tx = stage.tx();
        tx.animate(animation);
        stage
            .compositor
            .commit(tx)
            .expect_err("the animation is rejected")
            .reason
    };

    let mut backwards = fade(effect, 0.2, 0.8, timing(100, FillMode::None));
    if let Keyframes::Scalar(frames) = &mut backwards.keyframes {
        frames[0].offset = 0.5;
    }
    assert_eq!(reject(&mut stage, backwards), RejectReason::InvalidItem);

    for (index, param) in [(1, FilterParam::Amount), (0, FilterParam::Offset)] {
        let animation = Animation {
            target: AnimationTarget::Filter {
                effect,
                index,
                param,
            },
            ..fade(effect, 0.2, 0.8, timing(100, FillMode::None))
        };
        assert_eq!(reject(&mut stage, animation), RejectReason::InvalidItem);
    }

    let mut tx = stage.tx();
    let gone = tx.create_effect(effect_node(1.0, &[]));
    tx.remove(gone);
    stage.commit(tx);
    assert_eq!(
        reject(
            &mut stage,
            fade(gone, 0.2, 0.8, timing(100, FillMode::None))
        ),
        RejectReason::StaleId
    );
}
