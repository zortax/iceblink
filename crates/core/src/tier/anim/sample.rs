//! Sampling a client's animations at the time of a frame.

use iceblink::{
    AnimationEvent, AnimationTarget, AnyId, ClientId, CompositorEvent, EffectId, EffectNode,
    FillMode, Matrix4, ScrollPhase, TransformId,
};
use smallvec::SmallVec;

use crate::motion::timing::{Phase, active_duration, sample};
use crate::motion::value::{Value, value_at};
use crate::tier::View;
use crate::tier::anim::effect::animated_effect;
use crate::tier::anim::{Animations, Base, Ends, Node, Running};
use crate::tier::scroll::Scrolls;

/// A node value for the scene to show.
#[derive(Clone, Debug)]
pub(crate) enum Write {
    Transform(TransformId, Matrix4),
    Effect(EffectId, EffectNode),
}

impl Animations {
    /// Samples every running animation at `now_ns`.
    ///
    /// A transform or effect node whose animated value differs from the scene's is added to
    /// `writes`. An animated scroll offset becomes its node's programmatic offset.
    ///
    /// Animations that reach their end emit `Finished`. Those that do not fill forwards are then
    /// dropped, and their nodes return to their base values.
    pub(crate) fn sample(
        &mut self,
        view: &View<'_>,
        now_ns: u64,
        ends: &mut Ends<'_>,
        scrolls: &mut Scrolls,
        writes: &mut Vec<Write>,
    ) {
        let mut touched: SmallVec<[Node; 4]> = core::mem::take(&mut self.dirty);
        let mut index = 0;
        while index < self.running.len() {
            let running = &mut self.running[index];
            if running.finished {
                // A held end value does not change.
                index += 1;
                continue;
            }
            let ended = advance(running, now_ns, ends.client, ends.events);
            let node = Node::of(&running.animation.target);
            if !touched.contains(&node) {
                touched.push(node);
            }
            if ended {
                let running = self.running.swap_remove(index);
                ends.ids.free(&[AnyId::Animation(running.id)]);
            } else {
                index += 1;
            }
        }

        for node in touched {
            self.write(node, view, scrolls, writes);
        }
    }

    /// Adds the write that shows `node` with its animated values, if it differs from the scene,
    /// and forgets the base of a node no animation drives any more.
    fn write(
        &mut self,
        node: Node,
        view: &View<'_>,
        scrolls: &mut Scrolls,
        writes: &mut Vec<Write>,
    ) {
        let driven = self
            .running
            .iter()
            .any(|running| Node::of(&running.animation.target) == node);

        let Some(base) = self.bases.get(&node) else {
            return;
        };
        match base {
            Base::Transform(id, local) => {
                let value = self
                    .running
                    .iter()
                    .filter(|running| running.animation.target == AnimationTarget::Transform(*id))
                    .find_map(|running| match running.value {
                        Some(Value::Matrix(matrix)) => Some(matrix),
                        _ => None,
                    })
                    .unwrap_or(*local);
                if let Some(record) = view.transforms.get(id.slot(), id.generation())
                    && record.node.local != value
                {
                    writes.push(Write::Transform(*id, value));
                }
            }
            Base::Effect(id, base) => {
                let value = animated_effect(base, id.slot(), &self.running);
                if let Some(record) = view.effects.get(id.slot(), id.generation())
                    && record.node != value
                {
                    writes.push(Write::Effect(*id, value));
                }
            }
            Base::Scroll => {
                if let Node::Scroll(slot) = node {
                    drive_offset(&self.running, slot, view, scrolls);
                }
            }
        }

        if !driven {
            self.bases.remove(&node);
        }
    }

    /// Returns the earliest time after `now_ns` at which a sample changes anything, given frames
    /// every `interval_ns`.
    pub(crate) fn wake(&self, now_ns: u64, interval_ns: u64) -> Option<u64> {
        self.running
            .iter()
            .filter_map(|running| {
                let start = running.start_ns.unwrap_or(now_ns);
                let begins = i128::from(start) + i128::from(running.animation.timing.delay_ns);
                if i128::from(now_ns) < begins {
                    return Some(begins.clamp(0, i128::from(u64::MAX)) as u64);
                }
                (!running.finished).then(|| now_ns.saturating_add(interval_ns.max(1)))
            })
            .min()
    }
}

/// Hands the sampled value of the animation of scroll node `slot` to its scroll state, which
/// shows it in place of the base offset, or the base offset again once no animation drives it.
fn drive_offset(running: &[Running], slot: u32, view: &View<'_>, scrolls: &mut Scrolls) {
    let animated = running
        .iter()
        .find_map(|running| match running.animation.target {
            AnimationTarget::ScrollOffset(id) if id.slot() == slot => {
                Some((id, running.value, running.finished))
            }
            _ => None,
        });
    let id = animated
        .map(|(id, ..)| id)
        .or_else(|| scrolls.nodes.get(&slot).map(|state| state.id));
    let value = animated.and_then(|(_, value, _)| match value {
        Some(Value::Vector(offset)) => Some(offset),
        _ => None,
    });

    let Some(id) = id else {
        return;
    };
    let Some(record) = view.scrolls.get(id.slot(), id.generation()) else {
        return;
    };

    let state = scrolls.state(id, record.node.offset);
    state.animated = animated.is_some_and(|(.., finished)| !finished);
    if state.programmatic != value {
        state.programmatic = value;
        state.cause = ScrollPhase::Programmatic;
        state.unsettled = true;
    }
    scrolls.wake(slot);
}

/// Samples one animation at `now_ns`, emits the events of the phases it entered, and reports
/// whether it ended.
fn advance(
    running: &mut Running,
    now_ns: u64,
    client: ClientId,
    events: &mut Vec<CompositorEvent>,
) -> bool {
    let timing = &running.animation.timing;
    let start = *running.start_ns.get_or_insert(now_ns);
    let sampled = sample(timing, i128::from(now_ns) - i128::from(start));
    let begins = i128::from(start) + i128::from(timing.delay_ns);
    let at = |offset: f64| (begins as f64 + offset).max(0.0) as u64;
    let mut emit = |event: AnimationEvent, at_ns: u64| {
        events.push(CompositorEvent::Animation {
            client,
            id: running.id,
            event,
            at_ns,
        });
    };

    if sampled.phase != Phase::Before && !running.started {
        running.started = true;
        emit(AnimationEvent::Started, at(0.0));
    }
    if sampled.phase == Phase::Active && sampled.iteration > running.iteration {
        emit(
            AnimationEvent::Iteration,
            at(sampled.iteration as f64 * timing.duration_ns as f64),
        );
    }
    running.iteration = sampled.iteration;

    if sampled.phase == Phase::After && !running.finished {
        running.finished = true;
        emit(AnimationEvent::Finished, at(active_duration(timing)));
    }

    let seconds = timing.duration_ns as f32 / 1e9;
    running.value = sampled
        .progress
        .and_then(|progress| value_at(&running.animation.keyframes, progress, seconds));
    sampled.phase == Phase::After && !matches!(timing.fill, FillMode::Forwards | FillMode::Both)
}
