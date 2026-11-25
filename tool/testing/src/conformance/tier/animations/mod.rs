//! Rules of compositor animations: sampling, easing, writes, replacement, cancellation, events
//! and waking.

mod ends;
mod events;
mod rejections;
mod sampling;

use std::sync::Arc;

use iceblink::smallvec::smallvec;
use iceblink::{
    Animation, AnimationEvent, AnimationId, AnimationStart, AnimationTarget, BlendMode, ChunkKey,
    CompositorEvent, Direction, Easing, EffectId, EffectNode, FillMode, Filter, Keyframe,
    Keyframes, Matrix4, OrderEntry, OrderNode, Timing, TransformId,
};

use crate::conformance::tier::stage::{Factory, Stage, identity, props, quads};

pub use self::ends::{
    a_new_animation_replaces_the_running_one, an_explicit_write_replaces_the_animation,
    cancelling_an_ended_animation_does_nothing, cancelling_returns_the_base_value,
    removing_the_target_cancels_its_animations, the_fill_decides_the_value_after_the_end,
};
pub use self::events::{
    a_delayed_animation_wakes_an_idle_output, events_follow_the_life_of_an_animation,
};
pub use self::rejections::malformed_animations_are_rejected;
pub use self::sampling::{
    a_filter_parameter_animates, a_sample_damages_what_the_equal_write_damages,
    a_transform_animates_by_decomposition, an_animation_is_sampled_at_the_present_time,
    timing_and_keyframe_easings_both_apply,
};

const MS: u64 = 1_000_000;

/// The chunk every animation stage draws.
const CHUNK: ChunkKey = ChunkKey::new(1, 0);

/// A stage drawing a quad through an effect node of opacity `base` with `filters`.
fn faded(factory: Factory<'_>, base: f32, filters: &[Filter]) -> (Stage, EffectId) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    let effect = tx.create_effect(effect_node(base, filters));
    let group = tx.create_order(OrderNode {
        effect: Some(effect),
    });
    quads(
        &mut tx,
        CHUNK,
        props(stage.space),
        &[[20.0, 20.0, 100.0, 100.0]],
    );
    tx.splice(group, 0, 0, &[OrderEntry::Chunk(CHUNK)]);
    stage.push(&mut tx, OrderEntry::Node(group));
    stage.commit(tx);
    stage.draw_at(0);
    (stage, effect)
}

/// An effect of `opacity` with `filters` that does not isolate otherwise.
fn effect_node(opacity: f32, filters: &[Filter]) -> EffectNode {
    EffectNode {
        clip: None,
        opacity,
        blend: BlendMode::NORMAL,
        filters: Arc::from(filters),
        isolate: false,
    }
}

/// A stage drawing a 50 by 50 quad at the origin of a transform node under the root.
fn moved(factory: Factory<'_>) -> (Stage, TransformId) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    let space = tx.create_transform(identity(Some(stage.space)));
    quads(&mut tx, CHUNK, props(space), &[[0.0, 0.0, 50.0, 50.0]]);
    stage.push(&mut tx, OrderEntry::Chunk(CHUNK));
    stage.commit(tx);
    stage.draw_at(0);
    (stage, space)
}

/// A linear timing of one iteration over `duration_ms` with `fill`.
fn timing(duration_ms: u64, fill: FillMode) -> Timing {
    Timing {
        duration_ns: duration_ms * MS,
        delay_ns: 0,
        iterations: 1.0,
        direction: Direction::Normal,
        fill,
        easing: Easing::Linear,
    }
}

/// Keyframes from `from` to `to`, eased by `easing` between them.
fn scalars(from: f32, to: f32, easing: Easing) -> Keyframes {
    Keyframes::Scalar(smallvec![
        Keyframe {
            offset: 0.0,
            value: from,
            easing,
        },
        Keyframe {
            offset: 1.0,
            value: to,
            easing: Easing::Linear,
        },
    ])
}

/// Keyframes from the identity to `to`.
fn matrices(to: Matrix4) -> Keyframes {
    Keyframes::Matrix(smallvec![
        Keyframe {
            offset: 0.0,
            value: Matrix4::IDENTITY,
            easing: Easing::Linear,
        },
        Keyframe {
            offset: 1.0,
            value: to,
            easing: Easing::Linear,
        },
    ])
}

/// An animation of `effect`'s opacity from `from` to `to` with `timing`, starting at time zero.
fn fade(effect: EffectId, from: f32, to: f32, timing: Timing) -> Animation {
    Animation {
        target: AnimationTarget::Opacity(effect),
        keyframes: scalars(from, to, Easing::Linear),
        timing,
        start: AnimationStart::At(0),
    }
}

/// Returns the opacity of the only group of the last frame, or 1 when it has none.
fn group_opacity(stage: &Stage) -> f32 {
    stage
        .frame
        .groups
        .first()
        .map_or(1.0, |group| group.opacity)
}

/// Asserts that `actual` is within 10⁻⁴ of `expected`.
#[track_caller]
fn close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-4,
        "{actual} is not {expected}"
    );
}

/// Returns the animation events in `events`, with their times.
fn animation_events(events: &[CompositorEvent]) -> Vec<(AnimationId, AnimationEvent, u64)> {
    events
        .iter()
        .filter_map(|event| match *event {
            CompositorEvent::Animation {
                id, event, at_ns, ..
            } => Some((id, event, at_ns)),
            _ => None,
        })
        .collect()
}

/// Returns the animation events in `events`, without their times.
fn kinds(events: &[CompositorEvent]) -> Vec<(AnimationId, AnimationEvent)> {
    animation_events(events)
        .into_iter()
        .map(|(id, event, _)| (id, event))
        .collect()
}
