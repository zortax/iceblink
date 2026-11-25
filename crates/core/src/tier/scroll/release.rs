//! The motion that follows when a gesture ends.

use iceblink::{ScrollId, ScrollPhase, ScrollPhysics, ScrollSource};

use crate::motion::physics::fling;
use crate::tier::View;
use crate::tier::scroll::Scrolls;
use crate::tier::scroll::motion::Motion;
use crate::tier::scroll::snap::snap_target;
use crate::tier::scroll::state::{Geometry, Return, behaviour};

/// The end of a gesture.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Release {
    /// The device velocity the fingers lifted at.
    pub(crate) velocity: [f32; 2],
    pub(crate) source: ScrollSource,
    pub(crate) at_ns: u64,
    /// The speed below which a fling ends, in device pixels per second.
    pub(crate) rest_speed: f64,
}

impl Scrolls {
    /// Starts the motion that follows `release` over `chain`.
    ///
    /// The motion depends on the node and the axis:
    ///
    /// - If a finger held the axis past its end, the displacement returns.
    /// - If the node has snap points, it springs to the one nearest where the motion would rest.
    /// - If the node is kinetic, last absorbed the gesture on the axis, and the release is faster
    ///   than the rest speed, it flings. When no node absorbed any, the first node of the chain
    ///   the axis scrolls flings.
    /// - Otherwise the axis rests where it is.
    pub(crate) fn release(
        &mut self,
        view: &View<'_>,
        chain: &[ScrollId],
        moved: [Option<u32>; 2],
        release: Release,
    ) {
        let Release {
            velocity,
            source,
            at_ns,
            rest_speed,
        } = release;

        let moved = [0, 1].map(|axis| {
            moved[axis].or_else(|| {
                chain
                    .iter()
                    .find(|id| {
                        view.scrolls
                            .get(id.slot(), id.generation())
                            .and_then(behaviour)
                            .is_some_and(|behaviour| behaviour.user_scrollable[axis])
                    })
                    .map(|id| id.slot())
            })
        });

        let speed = f64::from(velocity[0].abs().max(velocity[1].abs()));
        let stop_s = fling::stop_time(speed, rest_speed);

        for &id in chain {
            let Some(record) = view.scrolls.get(id.slot(), id.generation()) else {
                continue;
            };
            let (Some(behaviour), Some(geometry)) = (behaviour(record), Geometry::of(view, record))
            else {
                continue;
            };

            let kinetic = behaviour.physics == ScrollPhysics::Kinetic;
            let moved_here = moved.iter().any(|slot| *slot == Some(id.slot()));
            let state = self.state(id, record.node.offset);
            let mut woken = false;
            for axis in 0..2 {
                let held = &mut state.axes[axis];
                let v = f64::from(velocity[axis]);
                if held.pull != 0.0 {
                    held.ret = Some(Return {
                        from: held.displacement,
                        speed: if kinetic { v } else { 0.0 },
                        start_ns: at_ns,
                    });
                    held.pull = 0.0;
                    woken = true;
                    continue;
                }

                let carries = kinetic && moved[axis] == Some(id.slot());
                let flings = carries && v != 0.0 && speed > rest_speed;
                let units = v / geometry.scale[axis];
                if moved_here && !behaviour.snap.is_empty() {
                    let current = state.exact(axis, geometry.max[axis]);
                    let projection = if flings {
                        current + fling::distance(units, stop_s)
                    } else {
                        current
                    };
                    if let Some(target) = snap_target(
                        behaviour,
                        axis,
                        projection,
                        geometry.max[axis],
                        geometry.extent[axis],
                    ) {
                        let origin = f64::from(state.origin()[axis]);
                        let target = target - origin;
                        state.axes[axis].motion = Motion::Snap {
                            target,
                            away: state.delta[axis] - target,
                            velocity: if carries { units } else { 0.0 },
                            start_ns: at_ns,
                        };
                        state.cause = ScrollPhase::Snap;
                        woken = true;
                        continue;
                    }
                }
                if flings {
                    state.axes[axis].motion = Motion::Fling {
                        from: state.delta[axis],
                        velocity: units,
                        start_ns: at_ns,
                        stop_s,
                    };
                    state.cause = ScrollPhase::Momentum;
                    woken = true;
                }
            }

            if woken || moved_here {
                state.source = source;
                state.unsettled = true;
                self.wake(id.slot());
            }
        }
    }
}
