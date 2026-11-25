//! Scroll gestures.

use crate::id::{ClientId, ScrollId};

/// One step of a scroll gesture, in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollGesture {
    /// Where the gesture happens.
    pub position: [f32; 2],
    /// How far the offset should increase.
    pub delta: [f32; 2],
    /// Where the step falls in the gesture.
    pub phase: GesturePhase,
    /// The device the gesture comes from.
    pub source: ScrollSource,
    /// When the step happened, in nanoseconds on the frame clock.
    pub time_ns: u64,
}

/// The phase of a scroll gesture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GesturePhase {
    /// The gesture started.
    Begin,
    /// The gesture continued.
    Update,
    /// The fingers lifted, moving at this velocity in device pixels per second.
    End {
        /// The velocity as `[x, y]`.
        velocity: [f32; 2],
    },
    /// The platform's own momentum continues the gesture.
    Momentum,
    /// One discrete step, such as a wheel notch or a key press.
    Step,
}

/// The device a scroll gesture comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollSource {
    /// A mouse wheel.
    Wheel,
    /// A touchpad.
    Touchpad,
    /// A touch screen.
    Touch,
    /// A keyboard.
    Keyboard,
}

/// Where a scroll gesture was routed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollRouting {
    /// The gesture moved, or will move, this scroll node.
    Handled {
        /// The node that takes the gesture's delta.
        node: ScrollId,
        /// The client that owns the node.
        client: ClientId,
    },
    /// No user-scrollable node under the position could take the delta.
    Unhandled,
}
