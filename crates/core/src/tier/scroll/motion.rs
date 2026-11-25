//! The motion of one axis of a scroll node's compositor delta.

use crate::motion::physics::{arrived, fling, step};
use crate::motion::spring::Spring;

/// The motion that moves one axis of a compositor delta, in the node's units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Motion {
    #[default]
    Idle,
    /// A kinetic step's curve from one delta to another.
    Curve { from: f64, to: f64, start_ns: u64 },
    /// A fling at `velocity` units per second, which stops `stop_s` seconds after its start.
    Fling {
        from: f64,
        velocity: f64,
        start_ns: u64,
        stop_s: f64,
    },
    /// A spring towards `target`, from `away` units off it at `velocity` units per second.
    Snap {
        target: f64,
        away: f64,
        velocity: f64,
        start_ns: u64,
    },
}

/// Where a motion has moved the delta at one time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Step {
    pub(crate) delta: f64,
    /// The velocity, in units per second.
    pub(crate) velocity: f64,
    /// Whether the motion has ended.
    pub(crate) done: bool,
}

impl Motion {
    /// Returns the delta, the velocity and whether the motion has ended at `now_ns`, for a node
    /// with `scale` device pixels per unit on the axis.
    ///
    /// A snap ends once it is within half a device pixel of its target and slower than
    /// `rest_speed` device pixels per second, and its delta is then the target.
    ///
    /// Returns `None` for no motion.
    pub(crate) fn at(&self, now_ns: u64, scale: f64, rest_speed: f64) -> Option<Step> {
        let step = self.sample(now_ns)?;
        let Motion::Snap { target, .. } = *self else {
            return Some(step);
        };

        let landed = arrived(
            (step.delta - target) * scale,
            step.velocity * scale,
            rest_speed,
        );
        Some(if landed {
            Step {
                delta: target,
                velocity: 0.0,
                done: true,
            }
        } else {
            step
        })
    }

    /// Returns the delta and the velocity at `now_ns`, and whether a curve or a fling has ended.
    ///
    /// Returns `None` for no motion.
    pub(crate) fn sample(&self, now_ns: u64) -> Option<Step> {
        match *self {
            Motion::Idle => None,
            Motion::Curve { from, to, start_ns } => {
                let elapsed = now_ns.saturating_sub(start_ns);
                let share = f64::from(step::share(elapsed));
                Some(Step {
                    delta: from + (to - from) * share,
                    velocity: 0.0,
                    done: elapsed >= step::DURATION_NS,
                })
            }
            Motion::Fling {
                from,
                velocity,
                start_ns,
                stop_s,
            } => {
                let seconds = seconds_since(start_ns, now_ns).min(stop_s);
                Some(Step {
                    delta: from + fling::distance(velocity, seconds),
                    velocity: fling::velocity(velocity, seconds),
                    done: seconds >= stop_s,
                })
            }
            Motion::Snap {
                target,
                away,
                velocity,
                start_ns,
            } => {
                let seconds = seconds_since(start_ns, now_ns);
                let (away, velocity) = Spring::SNAP.at(away, velocity, seconds);
                Some(Step {
                    delta: target + away,
                    velocity,
                    done: false,
                })
            }
        }
    }

    /// Moves the motion's deltas by `by`, after the delta it moves was rebased by as much.
    pub(crate) fn shift(&mut self, by: f64) {
        match self {
            Motion::Idle => {}
            Motion::Curve { from, to, .. } => {
                *from += by;
                *to += by;
            }
            Motion::Fling { from, .. } => *from += by,
            Motion::Snap { target, .. } => *target += by,
        }
    }
}

/// Returns the seconds from `start_ns` to `now_ns`, zero when `now_ns` is earlier.
pub(crate) fn seconds_since(start_ns: u64, now_ns: u64) -> f64 {
    now_ns.saturating_sub(start_ns) as f64 / 1e9
}
