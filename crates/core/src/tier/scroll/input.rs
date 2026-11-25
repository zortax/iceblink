//! Taking one step of a scroll gesture.

use iceblink::{GesturePhase, OutputId, ScrollGesture, ScrollId, ScrollPhysics, ScrollSource};
use smallvec::SmallVec;

use crate::motion::physics::{DEAD_DELTA, bounce, rest_speed};
use crate::tier::View;
use crate::tier::scroll::distribute::{Kind, Moved, Push};
use crate::tier::scroll::release::Release;
use crate::tier::scroll::state::{Geometry, behaviour};
use crate::tier::scroll::{Latch, Scrolls};

/// One step of a gesture on an output, with the scroll nodes under its position when it is
/// routed afresh.
pub(crate) struct Input {
    pub(crate) output: OutputId,
    pub(crate) gesture: ScrollGesture,
    /// The scroll nodes under the position, innermost first.
    pub(crate) candidates: SmallVec<[ScrollId; 4]>,
    /// The time the step takes effect: its own time, or the last sampled time when that is later.
    pub(crate) at_ns: u64,
    /// The refresh interval of the output's latest frame, zero before its first.
    pub(crate) interval_ns: u64,
}

impl Scrolls {
    /// Reports whether a step in `phase` of the gesture of `output` is routed afresh.
    ///
    /// `Begin` and `Step` always are, and the other phases are while the gesture has latched onto
    /// no node.
    pub(crate) fn routes(&self, output: OutputId, phase: GesturePhase) -> bool {
        let latched = self
            .latches
            .iter()
            .any(|(of, latch)| *of == output && !latch.chain.is_empty());
        match phase {
            GesturePhase::Begin | GesturePhase::Step => true,
            GesturePhase::Update | GesturePhase::Momentum | GesturePhase::End { .. } => !latched,
        }
    }

    /// Takes one step of a gesture and returns the node it moved or will move.
    ///
    /// The nodes whose offset animation the step ended are added to `interrupted`.
    pub(crate) fn input(
        &mut self,
        view: &View<'_>,
        input: Input,
        interrupted: &mut SmallVec<[u32; 2]>,
    ) -> Option<ScrollId> {
        let Input {
            output,
            gesture,
            candidates,
            at_ns,
            interval_ns,
        } = input;
        let delta = gesture.delta;

        let mut latch = self.latch_for(view, output, &gesture, &candidates, at_ns);

        let kind = match gesture.phase {
            GesturePhase::Momentum => Kind::Momentum,
            GesturePhase::Step => Kind::Step,
            GesturePhase::Begin | GesturePhase::Update | GesturePhase::End { .. } => Kind::Update,
        };
        let chain = latch.chain.clone();
        self.distribute(
            view,
            &chain,
            delta,
            Push {
                kind,
                finger: latch.finger,
                source: gesture.source,
                at_ns,
            },
            Moved {
                nodes: &mut latch.moved,
                interrupted,
            },
        );

        let release = |velocity| Release {
            velocity,
            source: gesture.source,
            at_ns,
            rest_speed: rest_speed(interval_ns),
        };
        match gesture.phase {
            GesturePhase::End { velocity } => {
                self.release(view, &chain, latch.moved, release(velocity));
                latch.finger = false;
            }
            GesturePhase::Step => {
                // An instant step's motion ends immediately, so it snaps.
                let instant: SmallVec<[ScrollId; 4]> = chain
                    .iter()
                    .copied()
                    .filter(|id| {
                        view.scrolls
                            .get(id.slot(), id.generation())
                            .and_then(behaviour)
                            .is_some_and(|behaviour| behaviour.physics == ScrollPhysics::Instant)
                    })
                    .collect();
                self.release(view, &instant, latch.moved, release([0.0; 2]));
            }
            GesturePhase::Begin | GesturePhase::Update | GesturePhase::Momentum => {}
        }

        latch.last_ns = at_ns;
        let node = latch.chain.first().copied();
        self.set_latch(output, Some(latch));
        node
    }

    /// Returns the latch a step of `gesture` moves, routed afresh over `candidates` where the
    /// step's phase asks for it.
    ///
    /// A `Begin` first stops the motion of every candidate.
    fn latch_for(
        &mut self,
        view: &View<'_>,
        output: OutputId,
        gesture: &ScrollGesture,
        candidates: &[ScrollId],
        at_ns: u64,
    ) -> Latch {
        let delta = gesture.delta;
        match gesture.phase {
            GesturePhase::Begin => {
                self.catch(view, candidates, at_ns);
                let mut latch = Latch {
                    finger: true,
                    ..Latch::default()
                };
                latch.chain = self.route(view, candidates, delta, gesture.source);
                latch
            }
            GesturePhase::Step => Latch {
                chain: self.route(view, candidates, delta, gesture.source),
                ..Latch::default()
            },
            GesturePhase::Update | GesturePhase::Momentum | GesturePhase::End { .. } => {
                let mut latch = self.latch(output).cloned().unwrap_or_default();
                if latch.chain.is_empty() {
                    // A release that finds no latched node is routed by where it flings.
                    let towards = match gesture.phase {
                        GesturePhase::End { velocity } if delta == [0.0; 2] => velocity,
                        _ => delta,
                    };
                    latch.chain = self.route(view, candidates, towards, gesture.source);
                }
                latch
            }
        }
    }

    /// Returns the part of `candidates` a gesture with device delta `delta` latches onto.
    ///
    /// Of the candidates that the gesture may scroll on an axis where the delta is nonzero, the
    /// first that can move by the delta takes it, with the rest outwards of it. When none can,
    /// the gesture latches onto them all if one of them bounces gestures from `source`, and onto
    /// none otherwise.
    fn route(
        &self,
        view: &View<'_>,
        candidates: &[ScrollId],
        delta: [f32; 2],
        source: ScrollSource,
    ) -> SmallVec<[ScrollId; 4]> {
        let chain: SmallVec<[ScrollId; 4]> = candidates
            .iter()
            .copied()
            .filter(|id| {
                view.scrolls
                    .get(id.slot(), id.generation())
                    .and_then(behaviour)
                    .is_some_and(|behaviour| {
                        (0..2).any(|axis| {
                            behaviour.user_scrollable[axis] && delta[axis].abs() >= DEAD_DELTA
                        })
                    })
            })
            .collect();

        if let Some(first) = chain.iter().position(|&id| self.can_move(view, id, delta)) {
            return chain[first..].iter().copied().collect();
        }

        let bounces = chain.iter().any(|id| {
            view.scrolls
                .get(id.slot(), id.generation())
                .and_then(behaviour)
                .is_some_and(|behaviour| behaviour.bounce.holds(source))
        });
        if bounces { chain } else { SmallVec::new() }
    }

    /// Stops the motion of every candidate a new gesture lands on, and lets the finger hold
    /// their displacements.
    fn catch(&mut self, view: &View<'_>, candidates: &[ScrollId], at_ns: u64) {
        for &id in candidates {
            let Some(record) = view.scrolls.get(id.slot(), id.generation()) else {
                continue;
            };
            let Some(geometry) = Geometry::of(view, record) else {
                continue;
            };
            let Some(state) = self
                .nodes
                .get_mut(&id.slot())
                .filter(|state| state.id == id)
            else {
                continue;
            };

            state.stop(at_ns);
            for (axis, held) in state.axes.iter_mut().enumerate() {
                if held.ret.take().is_some() {
                    held.pull = bounce::pull(held.displacement, geometry.device_extent(axis));
                }
            }
        }
    }
}
