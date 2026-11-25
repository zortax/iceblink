//! Advancing scroll motion to the time of a frame.

use iceblink::{ClientId, CompositorEvent, ScrollId, ScrollPhase};
use smallvec::SmallVec;

use crate::motion::physics::{arrived, bounce};
use crate::tier::View;
use crate::tier::scroll::Scrolls;
use crate::tier::scroll::motion::{Motion, seconds_since};
use crate::tier::scroll::snap::snap_target;
use crate::tier::scroll::state::{
    Geometry, Return, ScrollState, applied, applied_delta, behaviour, range,
};

/// A scroll node's offset and overscroll displacement for the scene to show.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Change {
    pub(crate) id: ScrollId,
    pub(crate) offset: [f32; 2],
    pub(crate) overscroll: [f32; 2],
}

impl Scrolls {
    /// Advances every moving node to `now_ns`, where returns and snaps arrive below `rest_speed`
    /// device pixels per second.
    ///
    /// A node whose shown offset or displacement differs from the scene's is added to `changes`.
    /// A node whose shown state differs from its last event emits a scroll event, and a node
    /// that came to rest emits one with `ScrollPhase::Settled`.
    pub(crate) fn advance(
        &mut self,
        view: &View<'_>,
        client: ClientId,
        now_ns: u64,
        rest_speed: f64,
        changes: &mut Vec<Change>,
        events: &mut Vec<CompositorEvent>,
    ) {
        let held: SmallVec<[u32; 4]> = self
            .latches
            .iter()
            .filter(|(_, latch)| latch.finger)
            .flat_map(|(_, latch)| latch.chain.iter().map(|id| id.slot()))
            .collect();

        let moving = core::mem::take(&mut self.moving);
        for slot in moving {
            let Some(state) = self.nodes.get_mut(&slot) else {
                continue;
            };
            let Some(record) = view.scrolls.get(slot, state.id.generation()) else {
                self.nodes.remove(&slot);
                continue;
            };
            let geometry = Geometry::of(view, record);
            let behaviour = behaviour(record);
            let bounces = behaviour.is_some_and(|behaviour| behaviour.bounce.holds(state.source));
            let max = range(&record.node);

            for (axis, &limit) in max.iter().enumerate() {
                let curved = step_axis(state, axis, now_ns, rest_speed, limit, geometry, bounces);
                if let (Some(behaviour), Some(geometry)) = (behaviour, geometry)
                    && curved
                {
                    let current = state.exact(axis, limit);
                    if let Some(target) =
                        snap_target(behaviour, axis, current, limit, geometry.extent[axis])
                    {
                        let target = target - f64::from(state.origin()[axis]);
                        state.axes[axis].motion = Motion::Snap {
                            target,
                            away: state.delta[axis] - target,
                            velocity: 0.0,
                            start_ns: now_ns,
                        };
                    }
                }
            }
            state.cause = cause(state);

            let scale = geometry.map(|geometry| geometry.scale);
            let shown = applied(state, max, scale);
            if shown != (record.node.offset, record.overscroll) {
                changes.push(Change {
                    id: state.id,
                    offset: shown.0,
                    overscroll: shown.1,
                });
            }

            let delta = applied_delta(state.delta, scale);
            let carried = delta.map(|axis| axis as f32);
            if shown != state.shown {
                state.shown = shown;
                state.unsettled = true;
                let serial = state.serials.issue(delta);
                events.push(scroll_event(client, state, serial, carried, state.cause));
            }

            if state.is_active() || held.contains(&slot) {
                self.moving.push(slot);
            } else if state.unsettled {
                state.unsettled = false;
                let serial = state.serials.issue(delta);
                events.push(scroll_event(
                    client,
                    state,
                    serial,
                    carried,
                    ScrollPhase::Settled,
                ));
            }
        }
    }
}

/// Advances the motion and the bounce return of one axis to `now_ns`, where they arrive below
/// `rest_speed` device pixels per second, and reports whether a step's curve ended.
fn step_axis(
    state: &mut ScrollState,
    axis: usize,
    now_ns: u64,
    rest_speed: f64,
    max: f32,
    geometry: Option<Geometry>,
    bounces: bool,
) -> bool {
    let scale = geometry.map_or(1.0, |geometry| geometry.scale[axis]);
    let origin = f64::from(state.origin()[axis]);
    let motion = state.axes[axis].motion;
    let mut curved = false;

    if let Some(step) = motion.at(now_ns, scale, rest_speed) {
        let offset = origin + step.delta;
        let limit = f64::from(max);
        if offset < 0.0 || offset > limit {
            state.delta[axis] = offset.clamp(0.0, limit) - origin;
            state.axes[axis].motion = Motion::Idle;
            curved = matches!(motion, Motion::Curve { .. });
            if bounces && matches!(motion, Motion::Fling { .. }) {
                let held = &mut state.axes[axis];
                held.ret = Some(Return {
                    from: held.displacement,
                    speed: step.velocity * scale,
                    start_ns: now_ns,
                });
            }
        } else {
            state.delta[axis] = step.delta;
            if step.done {
                state.axes[axis].motion = Motion::Idle;
                curved = matches!(motion, Motion::Curve { .. });
            }
        }
    }

    let held = &mut state.axes[axis];
    if let Some(ret) = held.ret {
        let (displacement, velocity) =
            bounce::returning(ret.from, ret.speed, seconds_since(ret.start_ns, now_ns));
        if arrived(displacement, velocity, rest_speed) {
            held.displacement = 0.0;
            held.ret = None;
        } else {
            held.displacement = displacement;
        }
    }

    if geometry.is_none() {
        // A node whose space does not scale the plane takes no input, so its motion ends.
        state.axes[axis] = Default::default();
    }
    curved
}

/// Returns what moves a node, from the motion under way.
fn cause(state: &ScrollState) -> ScrollPhase {
    if state.programmatic.is_some() {
        return ScrollPhase::Programmatic;
    }

    let motions = state.axes.iter().map(|axis| axis.motion);
    if motions
        .clone()
        .any(|motion| matches!(motion, Motion::Snap { .. }))
    {
        ScrollPhase::Snap
    } else if motions
        .clone()
        .any(|motion| matches!(motion, Motion::Fling { .. }))
        || state.axes.iter().any(|axis| axis.ret.is_some())
    {
        ScrollPhase::Momentum
    } else if motions
        .clone()
        .any(|motion| matches!(motion, Motion::Curve { .. }))
    {
        ScrollPhase::Step
    } else {
        state.cause
    }
}

/// Returns the scroll event for a node's shown state.
fn scroll_event(
    client: ClientId,
    state: &ScrollState,
    serial: u64,
    delta: [f32; 2],
    phase: ScrollPhase,
) -> CompositorEvent {
    CompositorEvent::Scroll {
        client,
        node: state.id,
        offset: state.shown.0,
        overscroll: state.shown.1,
        serial,
        delta,
        phase,
    }
}
