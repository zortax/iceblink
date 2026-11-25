//! Rules of how animations end: explicit writes, replacement, cancellation, fill and the removal
//! of their target.

use iceblink::{
    Animation, AnimationEvent, AnimationStart, AnimationTarget, Built, FillMode, Matrix4,
};

use crate::conformance::tier::animations::{
    CHUNK, MS, animation_events, close, effect_node, fade, faded, group_opacity, kinds, matrices,
    moved, timing,
};
use crate::conformance::tier::stage::Factory;

/// An explicit write ends every animation of its target with `Replaced`, and the written value
/// stays.
pub fn an_explicit_write_replaces_the_animation(factory: Factory<'_>) {
    let (mut stage, effect) = faded(factory, 0.5, &[]);
    let mut tx = stage.tx();
    let id = tx.animate(fade(effect, 0.2, 0.8, timing(100, FillMode::Forwards)));
    stage.commit(tx);
    stage.draw_at(25 * MS);
    stage.events();

    let mut tx = stage.tx();
    tx.set_effect(effect, effect_node(0.3, &[]));
    stage.commit(tx);
    let events = animation_events(&stage.events());
    assert_eq!(events.len(), 1);
    assert_eq!((events[0].0, events[0].1), (id, AnimationEvent::Replaced));
    stage.draw_at(50 * MS);
    close(group_opacity(&stage), 0.3);
    stage.draw_at(200 * MS);
    close(group_opacity(&stage), 0.3);
}

/// A new animation of a target replaces the running one, which emits `Replaced`, and the commit
/// reports the new one.
pub fn a_new_animation_replaces_the_running_one(factory: Factory<'_>) {
    let (mut stage, effect) = faded(factory, 0.5, &[]);
    let mut tx = stage.tx();
    let first = tx.animate(fade(effect, 0.2, 0.8, timing(100, FillMode::None)));
    stage.commit(tx);
    stage.draw_at(25 * MS);
    stage.events();

    let mut tx = stage.tx();
    let second = tx.animate(fade(effect, 0.4, 0.4, timing(100, FillMode::None)));
    let report = stage.compositor.commit(tx).expect("the animation commits");
    assert_eq!(report.animations.as_slice(), &[second]);
    assert_eq!(
        kinds(&stage.events()),
        vec![(first, AnimationEvent::Replaced)]
    );
    stage.draw_at(50 * MS);
    close(group_opacity(&stage), 0.4);
}

/// Cancelling an animation emits `Cancelled` and returns the target to its base value.
pub fn cancelling_returns_the_base_value(factory: Factory<'_>) {
    let (mut stage, effect) = faded(factory, 0.5, &[]);
    let mut tx = stage.tx();
    let id = tx.animate(fade(effect, 0.2, 0.8, timing(100, FillMode::Forwards)));
    stage.commit(tx);
    stage.draw_at(25 * MS);
    stage.events();

    let mut tx = stage.tx();
    tx.cancel_animation(id);
    stage.commit(tx);
    assert_eq!(
        kinds(&stage.events()),
        vec![(id, AnimationEvent::Cancelled)]
    );
    assert_eq!(stage.draw_at(50 * MS), Built::Frame);
    close(group_opacity(&stage), 0.5);
}

/// A forwards fill holds the end value after the active phase, and without one the target
/// returns to its base value. Either way the output is then idle.
pub fn the_fill_decides_the_value_after_the_end(factory: Factory<'_>) {
    for (fill, after) in [(FillMode::Forwards, 0.8), (FillMode::None, 0.5)] {
        let (mut stage, effect) = faded(factory, 0.5, &[]);
        let mut tx = stage.tx();
        tx.animate(fade(effect, 0.2, 0.8, timing(100, fill)));
        stage.commit(tx);
        stage.draw_at(25 * MS);
        assert_eq!(stage.draw_at(150 * MS), Built::Frame);
        close(group_opacity(&stage), after);
        assert_eq!(stage.draw_at(300 * MS), Built::Idle { wake_ns: None });
    }
}

/// Removing an animation's target cancels the animation, also in the commit that started it.
pub fn removing_the_target_cancels_its_animations(factory: Factory<'_>) {
    let (mut stage, space) = moved(factory);
    let mut tx = stage.tx();
    let id = tx.animate(Animation {
        target: AnimationTarget::Transform(space),
        keyframes: matrices(Matrix4::translation(10.0, 0.0, 0.0)),
        timing: timing(100, FillMode::None),
        start: AnimationStart::Next,
    });
    stage.commit(tx);
    stage.draw_at(16 * MS);
    stage.events();

    let mut tx = stage.tx();
    tx.splice(stage.root, 0, 1, &[]);
    tx.remove_chunk(CHUNK);
    tx.remove(space);
    stage.commit(tx);
    assert_eq!(
        kinds(&stage.events()),
        vec![(id, AnimationEvent::Cancelled)]
    );

    let mut tx = stage.tx();
    let brief = tx.create_effect(effect_node(0.5, &[]));
    let id = tx.animate(fade(brief, 0.2, 0.8, timing(100, FillMode::None)));
    tx.remove(brief);
    let report = stage.compositor.commit(tx).expect("the commit applies");
    assert_eq!(report.animations.as_slice(), &[id]);
    assert_eq!(
        kinds(&stage.events()),
        vec![(id, AnimationEvent::Cancelled)]
    );
}

/// Cancelling or removing an animation that already ended commits and changes nothing.
pub fn cancelling_an_ended_animation_does_nothing(factory: Factory<'_>) {
    let (mut stage, effect) = faded(factory, 0.5, &[]);
    let mut tx = stage.tx();
    let id = tx.animate(fade(effect, 0.2, 0.8, timing(100, FillMode::None)));
    stage.commit(tx);
    stage.draw_at(50 * MS);
    stage.draw_at(150 * MS);
    stage.events();

    let mut tx = stage.tx();
    tx.cancel_animation(id);
    tx.remove(id);
    stage.commit(tx);
    assert!(animation_events(&stage.events()).is_empty());
    assert_eq!(stage.draw_at(200 * MS), Built::Idle { wake_ns: None });
}
