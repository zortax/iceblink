//! Which animations are well formed.

use iceblink::smallvec::smallvec;
use iceblink::{
    Animation, AnimationStart, AnimationTarget, ClientId, Direction, Easing, EffectId, FillMode,
    FilterParam, IdSpace, Keyframe, Keyframes, Timing,
};
use iceblink_core::motion::check::animation_valid;

fn effect() -> EffectId {
    IdSpace::new(ClientId(1)).mint()
}

fn opacity(keyframes: Keyframes) -> Animation {
    Animation {
        target: AnimationTarget::Opacity(effect()),
        keyframes,
        timing: Timing {
            duration_ns: 1,
            delay_ns: 0,
            iterations: 1.0,
            direction: Direction::Normal,
            fill: FillMode::None,
            easing: Easing::Linear,
        },
        start: AnimationStart::Next,
    }
}

fn scalar(offsets: &[f32]) -> Keyframes {
    Keyframes::Scalar(
        offsets
            .iter()
            .map(|&offset| Keyframe {
                offset,
                value: 1.0,
                easing: Easing::Linear,
            })
            .collect(),
    )
}

#[test]
fn keyframes_run_from_zero_to_one_in_order() {
    assert!(animation_valid(&opacity(scalar(&[0.0, 0.5, 1.0]))));
    assert!(!animation_valid(&opacity(scalar(&[0.0]))));
    assert!(!animation_valid(&opacity(scalar(&[0.1, 1.0]))));
    assert!(!animation_valid(&opacity(scalar(&[0.0, 0.6, 0.5, 1.0]))));
}

#[test]
fn keyframes_match_the_target() {
    let vector = Keyframes::Vector(smallvec![
        Keyframe {
            offset: 0.0,
            value: [0.0; 2],
            easing: Easing::Linear
        },
        Keyframe {
            offset: 1.0,
            value: [1.0; 2],
            easing: Easing::Linear
        },
    ]);
    assert!(!animation_valid(&opacity(vector.clone())));
    let mut offset = opacity(vector);
    offset.target = AnimationTarget::Filter {
        effect: effect(),
        index: 0,
        param: FilterParam::Offset,
    };
    assert!(animation_valid(&offset));
}

#[test]
fn numbers_are_finite_and_easings_well_formed() {
    let mut animation = opacity(scalar(&[0.0, 1.0]));
    animation.timing.iterations = f32::INFINITY;
    assert!(animation_valid(&animation));
    animation.timing.iterations = -1.0;
    assert!(!animation_valid(&animation));
    animation.timing.iterations = 1.0;
    animation.timing.easing = Easing::Spring {
        mass: 0.0,
        stiffness: 1.0,
        damping: 1.0,
        velocity: 0.0,
    };
    assert!(!animation_valid(&animation));
}
