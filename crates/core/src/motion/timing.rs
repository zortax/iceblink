//! The timing model of Web Animations Level 1, which turns local time into iteration progress.

use iceblink::{Direction, FillMode, Timing};

use crate::motion::easing::ease;

/// Where an animation's local time lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Before the delay has passed.
    Before,
    /// Inside the active duration.
    Active,
    /// Past the active duration.
    After,
}

/// An animation's timing at one local time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Where the local time lies.
    pub phase: Phase,
    /// The index of the current iteration, counted from zero.
    pub iteration: u64,
    /// The eased progress through the current iteration, or `None` when the animation applies
    /// no value.
    pub progress: Option<f32>,
}

/// Samples `timing` at `elapsed_ns` after the animation's start.
///
/// The local time `t` is `elapsed_ns - delay_ns`. Before zero the animation is in its before
/// phase, at or past `duration_ns × iterations` in its after phase, and active in between:
///
/// - In the active phase, the iteration is `floor(t / duration_ns)` and the progress
///   `(t mod duration_ns) / duration_ns`.
/// - In the before phase, the progress is that of local time zero when `fill` is `Backwards` or
///   `Both`, and there is none otherwise.
/// - In the after phase, the progress is that of the end of the active duration when `fill` is
///   `Forwards` or `Both`, and there is none otherwise.
///
/// The direction then reverses the progress of reversed iterations, and `timing.easing` maps it.
pub fn sample(timing: &Timing, elapsed_ns: i128) -> Sample {
    let local = elapsed_ns - i128::from(timing.delay_ns);
    let duration = timing.duration_ns as f64;
    let iterations = f64::from(timing.iterations.max(0.0));
    let active = active_duration(timing);

    let (phase, overall) = if local < 0 {
        (Phase::Before, 0.0)
    } else if (local as f64) >= active {
        let overall = if duration == 0.0 {
            iterations
        } else {
            active / duration
        };
        (Phase::After, overall)
    } else {
        (Phase::Active, local as f64 / duration)
    };

    let fills = match phase {
        Phase::Before => matches!(timing.fill, FillMode::Backwards | FillMode::Both),
        Phase::Active => true,
        Phase::After => matches!(timing.fill, FillMode::Forwards | FillMode::Both),
    };

    let (iteration, simple) = split(overall, phase, iterations);
    if !fills {
        return Sample {
            phase,
            iteration,
            progress: None,
        };
    }

    let reversed = match timing.direction {
        Direction::Normal => false,
        Direction::Reverse => true,
        Direction::Alternate => iteration % 2 == 1,
        Direction::AlternateReverse => iteration % 2 == 0,
    };
    let directed = if reversed { 1.0 - simple } else { simple };
    let seconds = (duration / 1e9) as f32;
    Sample {
        phase,
        iteration,
        progress: Some(ease(&timing.easing, directed as f32, seconds)),
    }
}

/// Returns the length of the active phase in nanoseconds, infinite for an animation that repeats
/// forever.
pub fn active_duration(timing: &Timing) -> f64 {
    let iterations = f64::from(timing.iterations.max(0.0));
    if timing.duration_ns == 0 || iterations == 0.0 {
        return 0.0;
    }
    timing.duration_ns as f64 * iterations
}

/// Splits overall progress into the iteration index and the progress through it.
///
/// At the end of a whole number of iterations the progress is 1 in the last iteration.
fn split(overall: f64, phase: Phase, iterations: f64) -> (u64, f64) {
    if !overall.is_finite() {
        return (u64::MAX, 1.0);
    }
    let simple = overall.fract();
    let iteration = overall.floor();
    if simple == 0.0 && phase == Phase::After && iterations != 0.0 && overall > 0.0 {
        return ((iteration - 1.0) as u64, 1.0);
    }
    (iteration as u64, simple)
}
