//! Rules of sampling animations: the present time, easings, transforms, filter parameters and
//! the damage of a sample.

use std::sync::Arc;

use iceblink::{
    Animation, AnimationStart, AnimationTarget, Built, Easing, FillMode, Filter, FilterParam,
    FrameFilter, Matrix4, StepJump, TransformId,
};

use crate::conformance::tier::animations::{
    MS, close, fade, faded, group_opacity, matrices, moved, scalars, timing,
};
use crate::conformance::tier::stage::Factory;

/// An animation is sampled at the present time of the frame being built.
pub fn an_animation_is_sampled_at_the_present_time(factory: Factory<'_>) {
    let (mut stage, effect) = faded(factory, 0.5, &[]);
    let mut tx = stage.tx();
    tx.animate(fade(effect, 0.2, 0.8, timing(100, FillMode::None)));
    stage.commit(tx);
    assert_eq!(stage.draw_at(25 * MS), Built::Frame);
    close(group_opacity(&stage), 0.35);
    assert_eq!(stage.draw_at(75 * MS), Built::Frame);
    close(group_opacity(&stage), 0.65);
}

/// The timing easing maps the iteration progress, and the keyframe easing maps the progress
/// within the pair of keyframes around it.
pub fn timing_and_keyframe_easings_both_apply(factory: Factory<'_>) {
    let (mut stage, effect) = faded(factory, 0.5, &[]);
    let mut tx = stage.tx();
    let mut stepped = timing(100, FillMode::None);
    stepped.easing = Easing::Steps {
        count: 4,
        jump: StepJump::End,
    };
    tx.animate(Animation {
        keyframes: scalars(
            0.1,
            0.9,
            Easing::LinearPoints(Arc::from([[0.0, 0.0], [0.5, 0.25], [1.0, 1.0]])),
        ),
        ..fade(effect, 0.0, 0.0, stepped)
    });
    stage.commit(tx);
    // Progress 0.6 steps to 0.5, which the keyframe easing maps to 0.25.
    stage.draw_at(60 * MS);
    close(group_opacity(&stage), 0.1 + 0.8 * 0.25);
}

/// A transform animation interpolates the matrices by decomposition, and hit tests see the
/// sampled matrix.
pub fn a_transform_animates_by_decomposition(factory: Factory<'_>) {
    let (mut stage, space) = moved(factory);
    let mut tx = stage.tx();
    tx.animate(Animation {
        target: AnimationTarget::Transform(space),
        keyframes: matrices(
            Matrix4::scale(2.0, 2.0, 1.0).then(&Matrix4::translation(100.0, 0.0, 0.0)),
        ),
        timing: timing(100, FillMode::None),
        start: AnimationStart::At(0),
    });
    stage.commit(tx);
    stage.draw_at(50 * MS);
    // Halfway, the quad is scaled by 1.5 and moved by 50.
    let hits = stage.compositor.hit_test(stage.output, [62.0, 15.0]);
    let hit = hits.first().expect("the moved quad is hit");
    assert!(
        (hit.local[0] - 8.0).abs() < 1e-3 && (hit.local[1] - 10.0).abs() < 1e-3,
        "{hit:?}"
    );
    assert!(
        stage
            .compositor
            .hit_test(stage.output, [10.0, 10.0])
            .is_empty()
    );
}

/// An animation of a filter parameter drives the parameter of the filter at its index.
pub fn a_filter_parameter_animates(factory: Factory<'_>) {
    let (mut stage, effect) = faded(factory, 1.0, &[Filter::Blur(1.0)]);
    let mut tx = stage.tx();
    tx.animate(Animation {
        target: AnimationTarget::Filter {
            effect,
            index: 0,
            param: FilterParam::Amount,
        },
        keyframes: scalars(2.0, 6.0, Easing::Linear),
        timing: timing(100, FillMode::None),
        start: AnimationStart::At(0),
    });
    stage.commit(tx);
    stage.draw_at(50 * MS);
    let filters = &stage
        .frame
        .groups
        .first()
        .expect("the blur draws a group")
        .filters;
    assert_eq!(filters.first(), Some(&FrameFilter::Blur(4.0)));
}

/// A sample damages and shifts what the explicit write of the same value damages and shifts, and
/// its frame reports that damage as the compositor's own.
pub fn a_sample_damages_what_the_equal_write_damages(factory: Factory<'_>) {
    let slide = |space: TransformId| Animation {
        target: AnimationTarget::Transform(space),
        keyframes: matrices(Matrix4::translation(100.0, 0.0, 0.0)),
        timing: timing(100, FillMode::None),
        start: AnimationStart::At(0),
    };
    let (mut animated, space) = moved(factory);
    let mut tx = animated.tx();
    tx.animate(slide(space));
    animated.commit(tx);
    assert_eq!(animated.draw_at(50 * MS), Built::Frame);

    let (mut written, space) = moved(factory);
    let mut tx = written.tx();
    tx.set_transform(space, Matrix4::translation(50.0, 0.0, 0.0));
    written.commit(tx);
    assert_eq!(written.draw_at(50 * MS), Built::Frame);
    assert_eq!(animated.frame.damage.draw, written.frame.damage.draw);
    assert_eq!(animated.frame.damage.present, written.frame.damage.present);
    assert_eq!(
        animated.frame.damage.driven, animated.frame.damage.draw,
        "the sample's damage is the compositor's own"
    );
    assert!(written.frame.damage.driven.is_empty());
    assert_eq!(animated.frame.shifts, written.frame.shifts);
}
