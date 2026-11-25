//! Distributing a gesture's delta along the latched chain of scroll nodes.

use iceblink::{ScrollId, ScrollPhase, ScrollPhysics, ScrollSource};
use smallvec::SmallVec;

use crate::motion::physics::{DEAD_DELTA, bounce};
use crate::tier::View;
use crate::tier::scroll::Scrolls;
use crate::tier::scroll::motion::Motion;
use crate::tier::scroll::state::{Axis, Geometry, Return, ScrollState, behaviour};

/// What kind of step a delta comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A finger's movement, or a smooth wheel's.
    Update,
    /// The platform's momentum.
    Momentum,
    /// One discrete step.
    Step,
}

/// How little of a delta is left over once it is absorbed, in device pixels.
const SPENT: f64 = 1e-6;

/// How a gesture step pushes the nodes it reaches.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Push {
    pub(crate) kind: Kind,
    /// Whether a finger holds the gesture.
    pub(crate) finger: bool,
    /// The device the gesture comes from.
    pub(crate) source: ScrollSource,
    /// When the step takes effect.
    pub(crate) at_ns: u64,
}

/// The nodes a delta moved and how.
pub(crate) struct Moved<'a> {
    /// The node that absorbed part of the delta last, per axis.
    pub(crate) nodes: &'a mut [Option<u32>; 2],
    /// The nodes whose offset animation the delta ended.
    pub(crate) interrupted: &'a mut SmallVec<[u32; 2]>,
}

impl Scrolls {
    /// Reports whether node `id` can move by the device delta `delta` on some axis.
    pub(crate) fn can_move(&self, view: &View<'_>, id: ScrollId, delta: [f32; 2]) -> bool {
        let Some(record) = view.scrolls.get(id.slot(), id.generation()) else {
            return false;
        };
        let (Some(behaviour), Some(geometry)) = (behaviour(record), Geometry::of(view, record))
        else {
            return false;
        };

        (0..2).any(|axis| {
            if !behaviour.user_scrollable[axis] || delta[axis].abs() < DEAD_DELTA {
                return false;
            }

            let offset = match self.nodes.get(&id.slot()).filter(|state| state.id == id) {
                Some(state) => destination(state, axis, geometry.max[axis]),
                None => f64::from(record.node.offset[axis]),
            };
            let room = if delta[axis] > 0.0 {
                f64::from(geometry.max[axis]) - offset
            } else {
                offset
            };
            room * geometry.scale[axis] > SPENT
        })
    }

    /// Moves the nodes of `chain` by the device delta `delta`.
    ///
    /// On each axis the first node absorbs what fits in its range, and a node that chains the
    /// push's source passes the rest to the next. What the last node reached cannot absorb is
    /// overscroll, which a node that bounces the source shows as a displacement, and another
    /// drops. Deltas below [`DEAD_DELTA`] on both axes move nothing.
    pub(crate) fn distribute(
        &mut self,
        view: &View<'_>,
        chain: &[ScrollId],
        delta: [f32; 2],
        push: Push,
        mut moved: Moved<'_>,
    ) {
        if delta.iter().all(|value| value.abs() < DEAD_DELTA) {
            return;
        }

        for (axis, &along) in delta.iter().enumerate() {
            if along != 0.0 {
                let rest = self.release_pull(view, chain, axis, f64::from(along));
                self.along(view, chain, axis, rest, push, &mut moved);
            }
        }
    }

    /// Moves the nodes of `chain` on `axis` by the device delta `rest`.
    fn along(
        &mut self,
        view: &View<'_>,
        chain: &[ScrollId],
        axis: usize,
        mut rest: f64,
        push: Push,
        moved: &mut Moved<'_>,
    ) {
        let mut last = None;
        for &id in chain {
            let Some(record) = view.scrolls.get(id.slot(), id.generation()) else {
                continue;
            };
            let (Some(behaviour), Some(geometry)) = (behaviour(record), Geometry::of(view, record))
            else {
                continue;
            };
            last = Some((id, geometry, behaviour.bounce.holds(push.source)));
            if behaviour.user_scrollable[axis] && rest.abs() > SPENT {
                let state = self.state(id, record.node.offset);
                if state.fold_programmatic() {
                    moved.interrupted.push(id.slot());
                }
                let units = rest / geometry.scale[axis];
                let absorbed =
                    if push.kind == Kind::Step && behaviour.physics == ScrollPhysics::Kinetic {
                        absorb_step(state, axis, units, geometry.max[axis], push.at_ns)
                    } else {
                        absorb(state, axis, units, geometry.max[axis])
                    };
                if absorbed != 0.0 {
                    state.cause = match push.kind {
                        Kind::Momentum => ScrollPhase::Momentum,
                        Kind::Step => ScrollPhase::Step,
                        Kind::Update => ScrollPhase::Gesture,
                    };
                    state.source = push.source;
                    state.unsettled = true;
                    moved.nodes[axis] = Some(id.slot());
                    self.wake(id.slot());
                }
                rest -= absorbed * geometry.scale[axis];
            }
            if rest.abs() <= SPENT || !behaviour.chain.holds(push.source) {
                break;
            }
        }

        if rest.abs() > SPENT
            && let Some((id, geometry, true)) = last
            && let Some(record) = view.scrolls.get(id.slot(), id.generation())
        {
            let state = self.state(id, record.node.offset);
            overscroll(
                &mut state.axes[axis],
                rest,
                geometry.device_extent(axis),
                push,
            );
            state.source = push.source;
            state.unsettled = true;
            self.wake(id.slot());
        }
    }

    /// Takes a delta against the direction of a held pull out of the pull first, and returns
    /// what is left of it.
    fn release_pull(
        &mut self,
        view: &View<'_>,
        chain: &[ScrollId],
        axis: usize,
        mut rest: f64,
    ) -> f64 {
        for &id in chain {
            let Some(state) = self
                .nodes
                .get_mut(&id.slot())
                .filter(|state| state.id == id)
            else {
                continue;
            };
            let held = &mut state.axes[axis];
            if held.pull == 0.0 || held.pull.signum() == rest.signum() {
                continue;
            }
            let Some(geometry) = view
                .scrolls
                .get(id.slot(), id.generation())
                .and_then(|record| Geometry::of(view, record))
            else {
                continue;
            };

            let taken = rest.abs().min(held.pull.abs());
            held.pull -= taken.copysign(held.pull);
            rest -= taken.copysign(rest);
            held.displacement = bounce::displacement(held.pull, geometry.device_extent(axis));
            self.wake(id.slot());
        }
        rest
    }
}

/// Shows the device delta `rest` past the end of a bouncing axis as a displacement.
///
/// A finger holds the pull of an update. A displacement no finger holds returns immediately.
fn overscroll(held: &mut Axis, rest: f64, extent: f64, push: Push) {
    if push.finger && push.kind == Kind::Update {
        held.pull += rest;
        held.ret = None;
        held.displacement = bounce::displacement(held.pull, extent);
        return;
    }

    let pull = bounce::pull(held.displacement, extent) + rest;
    let from = bounce::displacement(pull, extent);
    held.displacement = from;
    held.ret = Some(Return {
        from,
        speed: 0.0,
        start_ns: push.at_ns,
    });
}

/// Moves a node's offset on `axis` by up to `units`, and returns how far it moved.
fn absorb(state: &mut ScrollState, axis: usize, units: f64, max: f32) -> f64 {
    let current = state.exact(axis, max);
    let target = (current + units).clamp(0.0, f64::from(max));
    let taken = target - current;
    state.delta[axis] += taken;
    taken
}

/// Moves the destination of a node's step curve on `axis` by up to `units`, restarting the curve
/// from where the offset is at `at_ns`, and returns how far the destination moved.
fn absorb_step(state: &mut ScrollState, axis: usize, units: f64, max: f32, at_ns: u64) -> f64 {
    let from = match state.axes[axis].motion.sample(at_ns) {
        Some(step) => step.delta,
        None => state.delta[axis],
    };

    let origin = f64::from(state.origin()[axis]);
    let destination = destination(state, axis, max);
    let target = (destination + units).clamp(0.0, f64::from(max));
    let taken = target - destination;

    if taken != 0.0 {
        state.delta[axis] = from;
        state.axes[axis].motion = Motion::Curve {
            from,
            to: target - origin,
            start_ns: at_ns,
        };
    }
    taken
}

/// Returns where a node's offset on `axis` is headed: the destination of its step curve, or the
/// current offset.
fn destination(state: &ScrollState, axis: usize, max: f32) -> f64 {
    match state.axes[axis].motion {
        Motion::Curve { to, .. } => {
            (f64::from(state.origin()[axis]) + to).clamp(0.0, f64::from(max))
        }
        _ => state.exact(axis, max),
    }
}
