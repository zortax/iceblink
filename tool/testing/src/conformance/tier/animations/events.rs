//! Rules of animation events and of waking an idle output.

use iceblink::{AnimationEvent, Built, FillMode};

use crate::conformance::tier::animations::{
    MS, animation_events, close, fade, faded, group_opacity, timing,
};
use crate::conformance::tier::stage::Factory;

/// `Started`, `Iteration` and `Finished` follow the animation's timing, stamped with the times
/// the phases began.
pub fn events_follow_the_life_of_an_animation(factory: Factory<'_>) {
    let (mut stage, effect) = faded(factory, 0.5, &[]);
    let mut tx = stage.tx();
    let mut repeated = timing(100, FillMode::None);
    repeated.iterations = 2.0;
    repeated.delay_ns = 50 * MS as i64;
    let id = tx.animate(fade(effect, 0.2, 0.8, repeated));
    stage.commit(tx);
    for at in [20, 100, 200, 300] {
        stage.draw_at(at * MS);
    }
    let events = animation_events(&stage.events());
    assert_eq!(
        events,
        vec![
            (id, AnimationEvent::Started, 50 * MS),
            (id, AnimationEvent::Iteration, 150 * MS),
            (id, AnimationEvent::Finished, 250 * MS),
        ]
    );
}

/// An output whose only change is an animation waiting for its delay is idle until the delay
/// ends.
pub fn a_delayed_animation_wakes_an_idle_output(factory: Factory<'_>) {
    let (mut stage, effect) = faded(factory, 0.5, &[]);
    let mut tx = stage.tx();
    let mut delayed = timing(100, FillMode::None);
    delayed.delay_ns = 500 * MS as i64;
    tx.animate(fade(effect, 0.2, 0.8, delayed));
    stage.commit(tx);
    assert_eq!(
        stage.draw_at(16 * MS),
        Built::Idle {
            wake_ns: Some(500 * MS)
        }
    );
    assert_eq!(stage.draw_at(525 * MS), Built::Frame);
    close(group_opacity(&stage), 0.35);
}
