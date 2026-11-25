//! The driver's frames: building those that are due, and when the next ones latch.

use iceblink::{ClientEvent, DriverStats, FrameClock, FrameOutcome, now_ns};

use crate::core::Driver;
use crate::mailbox::Letter;
use crate::options::Pacing;
use crate::output::{Drawn, PresenterState};

impl Driver {
    /// Builds the frame of every output that wants one by `clock.now_ns`, at `clock`, if the
    /// driver is stepped.
    pub fn step(&mut self, clock: FrameClock) {
        if self.options.pacing != Pacing::Stepped {
            return;
        }
        self.steps += 1;
        for index in 0..self.outputs.len() {
            let output = &mut self.outputs[index];
            if output.schedule.due_by(clock.now_ns) {
                let before = output.state;
                let drawn = output.draw_stepped(&mut *self.compositor, clock);
                self.after_frame(index, drawn, before);
            }
        }
    }

    /// Runs every frame that latched by `now_ns`, if the driver is paced.
    ///
    /// Feedback that may show an output's newest frame at an earlier refresh than planned is
    /// read first, so a frame it moves earlier runs at its new latch.
    pub fn run_due(&mut self, now_ns: u64) {
        if self.options.pacing != Pacing::Paced {
            return;
        }
        for index in 0..self.outputs.len() {
            let output = &mut self.outputs[index];
            output.check_early(now_ns);
            if output.schedule.due().is_some_and(|due| due <= now_ns) {
                let before = output.state;
                let drawn = output.draw_paced(&mut *self.compositor, iceblink::now_ns());
                self.after_frame(index, drawn, before);
            }
        }
    }

    /// Returns when the next frame latches, or `None` when no output wants a frame.
    pub fn next_due(&self) -> Option<u64> {
        self.outputs
            .iter()
            .filter_map(|output| output.schedule.due())
            .min()
    }

    /// Returns when a paced driver next has work, which is the next latch or the first time
    /// feedback may show an output's newest frame at an earlier refresh than planned, whichever
    /// comes first.
    ///
    /// Such feedback can move a due frame earlier.
    pub fn next_wake(&self) -> Option<u64> {
        self.outputs
            .iter()
            .flat_map(|output| [output.schedule.due(), output.schedule.early()])
            .flatten()
            .min()
    }

    /// Reports whether a paced output's frame latched by `now_ns`, other than a frame an idle
    /// output latched immediately for a call.
    pub fn latched(&self, now_ns: u64) -> bool {
        self.options.pacing == Pacing::Paced
            && self.outputs.iter().any(|output| {
                !output.schedule.at_once() && output.schedule.due().is_some_and(|due| due <= now_ns)
            })
    }

    /// Returns a snapshot of the driver and its compositor.
    pub fn stats(&self) -> DriverStats {
        let next = self
            .outputs
            .iter()
            .filter_map(|output| Some((output.schedule.due()?, output.schedule.display()?)))
            .min();
        let now = now_ns();
        let reach = self
            .outputs
            .iter()
            .filter_map(|output| output.reach(now))
            .min();
        DriverStats {
            next_due_ns: next.map(|(due, _)| due),
            next_display_ns: next.map(|(_, display)| display),
            commit_due_ns: reach.map(|(due, _)| due),
            commit_display_ns: reach.map(|(_, display)| display),
            frames_built: self.frames_built,
            steps: self.steps,
            counters: *self.compositor.counters(),
        }
    }

    /// Counts a frame of output `index`, which had presenter state `before`, and addresses the
    /// events it caused.
    fn after_frame(&mut self, index: usize, drawn: Option<Drawn>, before: PresenterState) {
        let output = &mut self.outputs[index];
        if let Some(drawn) = drawn {
            if drawn.built {
                self.frames_built += 1;
            }
            for &client in &output.watchers {
                let event = ClientEvent::Drawn {
                    output: output.id,
                    frame: drawn.frame,
                    counters: drawn.counters,
                    outcome: drawn.outcome,
                };
                self.letters.push(Letter {
                    client,
                    event,
                    wakes: false,
                });
            }
            if matches!(drawn.outcome, FrameOutcome::Presented(_)) {
                for client in output.shown.drain(..) {
                    let event = ClientEvent::Shown {
                        output: output.id,
                        frame: drawn.frame,
                    };
                    self.letters.push(Letter {
                        client,
                        event,
                        wakes: true,
                    });
                }
            }
        }

        if output.state != before {
            for &client in &output.presenter {
                let event = ClientEvent::PresenterChanged {
                    output: output.id,
                    starved: output.state.starved,
                    occluded: output.state.occluded,
                };
                self.letters.push(Letter {
                    client,
                    event,
                    wakes: true,
                });
            }
        }
        self.route_events();
    }
}
