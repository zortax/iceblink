//! The gesture steps frames of the driven list start with.

use iceblink::{GesturePhase, ScrollGesture, ScrollSource};

use crate::conformance::end_to_end::drive::Drive;

/// One step of the input a frame starts with.
#[derive(Clone, Copy)]
pub(crate) enum Input {
    /// A wheel step by a delta.
    Wheel(f32),
    /// A touchpad gesture of one update, released at a velocity.
    Swipe(f32, f32),
    /// Fingers landing on the touchpad.
    Press,
    /// One update of the fingers on the touchpad.
    Drag(f32),
    /// The fingers lifting at a velocity.
    Lift(f32),
    /// No input.
    Nothing,
}

/// The gesture steps of one frame.
#[derive(Clone, Copy)]
pub(crate) struct Turn {
    /// The step sent before the client's commit.
    pub(crate) before: Input,
    /// The step sent after the client's commit.
    pub(crate) after: Input,
}

impl Turn {
    const NOTHING: Self = Self::before(Input::Nothing);

    const fn before(input: Input) -> Self {
        Self {
            before: input,
            after: Input::Nothing,
        }
    }
}

/// Wheel glides, then touchpad swipes with flings and bounces, all before the client's commits.
pub(crate) fn glides() -> Vec<Turn> {
    let mut turns = Vec::new();
    for lines in [40.0, 40.0, 80.0, -40.0] {
        turns.push(Turn::before(Input::Wheel(lines)));
        turns.extend([Turn::NOTHING; 7]);
    }
    for (delta, velocity) in [(12.0, 0.0), (-7.0, 0.0), (20.0, 1500.0), (30.0, 4000.0)] {
        turns.push(Turn::before(Input::Swipe(delta, velocity)));
        turns.extend([Turn::NOTHING; 20]);
    }
    turns.push(Turn::before(Input::Swipe(-40.0, -6000.0)));
    turns.extend([Turn::NOTHING; 40]);
    turns
}

/// Touchpad drags with an update before and after each of the client's commits, released at
/// rest and into a fling.
pub(crate) fn drags() -> Vec<Turn> {
    let drag = |before: f32, after: f32| Turn {
        before: Input::Drag(before),
        after: Input::Drag(after),
    };
    let mut turns = vec![Turn::before(Input::Press)];
    turns.extend([drag(7.0, 30.0); 6]);
    turns.push(Turn::before(Input::Lift(0.0)));
    turns.extend([Turn::NOTHING; 3]);
    turns.push(Turn::before(Input::Press));
    turns.extend([drag(-9.0, -25.0); 4]);
    turns.push(Turn::before(Input::Lift(-1200.0)));
    turns.extend([Turn::NOTHING; 20]);
    turns
}

/// Sends the gesture steps of `input` at `time_ns` to `drive`.
pub(crate) fn hand(drive: &mut Drive, input: Input, time_ns: u64) {
    let step = |delta: f32, phase: GesturePhase, source: ScrollSource| ScrollGesture {
        position: [50.0, 60.0],
        delta: [0.0, delta],
        phase,
        source,
        time_ns,
    };
    let touchpad = ScrollSource::Touchpad;
    let lift = |velocity: f32| GesturePhase::End {
        velocity: [0.0, velocity],
    };
    match input {
        Input::Wheel(delta) => {
            drive.scroll_input(step(delta, GesturePhase::Step, ScrollSource::Wheel))
        }
        Input::Swipe(delta, velocity) => {
            drive.scroll_input(step(0.0, GesturePhase::Begin, touchpad));
            drive.scroll_input(step(delta, GesturePhase::Update, touchpad));
            drive.scroll_input(step(0.0, lift(velocity), touchpad));
        }
        Input::Press => drive.scroll_input(step(0.0, GesturePhase::Begin, touchpad)),
        Input::Drag(delta) => drive.scroll_input(step(delta, GesturePhase::Update, touchpad)),
        Input::Lift(velocity) => drive.scroll_input(step(0.0, lift(velocity), touchpad)),
        Input::Nothing => {}
    }
}
