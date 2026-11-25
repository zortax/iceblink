//! Winit wheel events as scroll gestures.

use std::collections::VecDeque;

use iceblink::{GesturePhase, ScrollGesture, ScrollSource};
use iceblink_driver::now_ns;
use winit::event::{MouseScrollDelta, TouchPhase};

/// How far one line of a wheel scrolls, in logical pixels.
const LINE: f32 = 40.0;

/// How far back the release velocity of a touchpad gesture looks.
const WINDOW_NS: u64 = 80_000_000;

/// The gesture a wheel or touchpad is performing, and its recent steps.
#[derive(Default)]
pub(crate) struct Gestures {
    /// The deltas of the current touchpad gesture within the velocity window, with their times.
    recent: VecDeque<(u64, [f32; 2])>,
}

impl Gestures {
    /// Returns the gesture of one winit wheel event at `position`, in device pixels.
    ///
    /// A wheel's lines become steps. Touchpad deltas follow the event's touch phase, and the
    /// release carries the velocity of the last 80 ms of the gesture.
    pub(crate) fn step(
        &mut self,
        delta: MouseScrollDelta,
        phase: TouchPhase,
        position: [f32; 2],
        scale: f32,
    ) -> ScrollGesture {
        let time_ns = now_ns();
        // Winit's deltas are positive when the content moves right and down.
        let (delta, phase, source) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (
                [-x * LINE * scale, -y * LINE * scale],
                GesturePhase::Step,
                ScrollSource::Wheel,
            ),
            MouseScrollDelta::PixelDelta(at) => {
                let delta = [-(at.x as f32), -(at.y as f32)];
                (
                    delta,
                    self.touch(phase, delta, time_ns),
                    ScrollSource::Touchpad,
                )
            }
        };
        ScrollGesture {
            position,
            delta,
            phase,
            source,
            time_ns,
        }
    }

    fn touch(&mut self, phase: TouchPhase, delta: [f32; 2], time_ns: u64) -> GesturePhase {
        match phase {
            TouchPhase::Started => {
                self.recent.clear();
                self.recent.push_back((time_ns, delta));
                GesturePhase::Begin
            }
            TouchPhase::Moved => {
                self.recent.push_back((time_ns, delta));
                self.recent.retain(|&(at, _)| at + WINDOW_NS >= time_ns);
                GesturePhase::Update
            }
            TouchPhase::Ended => GesturePhase::End {
                velocity: self.velocity(time_ns),
            },
            TouchPhase::Cancelled => GesturePhase::End { velocity: [0.0; 2] },
        }
    }

    /// Returns the velocity of the recent deltas, in device pixels per second.
    fn velocity(&mut self, now_ns: u64) -> [f32; 2] {
        self.recent.retain(|&(at, _)| at + WINDOW_NS >= now_ns);
        let Some(&(first, _)) = self.recent.front() else {
            return [0.0; 2];
        };
        let seconds = (now_ns.saturating_sub(first) as f32 / 1e9).max(0.008);
        let moved = self.recent.drain(..).fold([0.0; 2], |sum, (_, delta)| {
            [sum[0] + delta[0], sum[1] + delta[1]]
        });
        [moved[0] / seconds, moved[1] / seconds]
    }
}
