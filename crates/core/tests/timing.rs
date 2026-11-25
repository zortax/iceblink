//! The timing model of Web Animations Level 1.

use iceblink::{Direction, Easing, FillMode, Timing};
use iceblink_core::motion::timing::{Phase, sample};

const MS: i128 = 1_000_000;

/// Returns a linear timing of 100 ms after a delay of 50 ms.
fn timing(iterations: f32, direction: Direction, fill: FillMode) -> Timing {
    Timing {
        duration_ns: 100_000_000,
        delay_ns: 50_000_000,
        iterations,
        direction,
        fill,
        easing: Easing::Linear,
    }
}

#[test]
fn the_before_phase_applies_the_start_value_only_with_a_backwards_fill() {
    let none = sample(&timing(1.0, Direction::Normal, FillMode::None), 20 * MS);
    assert_eq!((none.phase, none.progress), (Phase::Before, None));

    let backwards = sample(
        &timing(1.0, Direction::Reverse, FillMode::Backwards),
        20 * MS,
    );
    assert_eq!(
        (backwards.phase, backwards.progress),
        (Phase::Before, Some(1.0))
    );
}

#[test]
fn the_active_phase_counts_iterations_and_reverses_alternate_ones() {
    let normal = sample(&timing(3.0, Direction::Alternate, FillMode::None), 175 * MS);
    assert_eq!(normal.phase, Phase::Active);
    assert_eq!(normal.iteration, 1);
    let progress = normal.progress.expect("an active animation has progress");
    assert!((progress - 0.75).abs() < 1e-6, "{progress}");

    let first = sample(
        &timing(3.0, Direction::AlternateReverse, FillMode::None),
        75 * MS,
    );
    assert_eq!(first.iteration, 0);
    assert!((first.progress.expect("active") - 0.75).abs() < 1e-6);
}

#[test]
fn the_after_phase_holds_the_end_value_only_with_a_forwards_fill() {
    let ended = sample(&timing(2.0, Direction::Normal, FillMode::None), 400 * MS);
    assert_eq!((ended.phase, ended.progress), (Phase::After, None));

    let held = sample(
        &timing(2.0, Direction::Normal, FillMode::Forwards),
        250 * MS,
    );
    assert_eq!(
        (held.phase, held.iteration, held.progress),
        (Phase::After, 1, Some(1.0))
    );

    let fraction = sample(&timing(1.5, Direction::Normal, FillMode::Both), 400 * MS);
    assert_eq!(fraction.iteration, 1);
    assert!((fraction.progress.expect("filled") - 0.5).abs() < 1e-6);
}

#[test]
fn an_animation_that_repeats_for_ever_never_ends() {
    let late = sample(
        &timing(f32::INFINITY, Direction::Normal, FillMode::None),
        1_000_000 * MS,
    );
    assert_eq!(late.phase, Phase::Active);
}

#[test]
fn the_timing_easing_maps_the_directed_progress() {
    let mut eased = timing(1.0, Direction::Reverse, FillMode::None);
    eased.easing = Easing::Steps {
        count: 2,
        jump: iceblink::StepJump::End,
    };
    // Reversed progress 0.8 steps down to 0.5.
    assert_eq!(sample(&eased, 70 * MS).progress, Some(0.5));
}
