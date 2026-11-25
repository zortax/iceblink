//! What a paced output learns of the display from its presenter's feedback.

mod refresh;

use std::collections::VecDeque;

use iceblink::{PresentFeedback, Presenter};

use crate::output::budget::Budget;
use crate::output::schedule::Schedule;

use self::refresh::Refresh;

/// How many presented frames wait for their feedback at most.
const WAITING: usize = 16;

/// The display an output presents to: its refreshes, and the presented frames that wait for
/// feedback.
#[derive(Debug)]
pub(crate) struct Display {
    /// The refresh interval the driver assumes until it measured one.
    nominal: u64,
    /// The refresh interval the newest feedback reported, if it knew one.
    reported: Option<u64>,
    refresh: Refresh,
    /// The presented frames that wait for feedback, oldest first.
    waiting: VecDeque<Waiting>,
    feedback: Vec<PresentFeedback>,
}

/// A presented frame that waits for feedback.
#[derive(Clone, Copy, Debug)]
struct Waiting {
    /// The number the presenter gave the present.
    present: u64,
    /// The display time the frame was built for.
    target: u64,
    /// Whether the frame was built for a refresh of the display and drawn within its budget, so
    /// that its arrival counts as a hit or a miss of the budget.
    judged: bool,
}

impl Display {
    /// Returns the display of an output whose refresh interval is assumed to be `nominal_ns`
    /// until it is measured.
    pub(crate) fn new(nominal_ns: u64) -> Self {
        Self {
            nominal: nominal_ns,
            reported: None,
            refresh: Refresh::default(),
            waiting: VecDeque::new(),
            feedback: Vec::new(),
        }
    }

    /// Notes that a frame built for display time `target` was presented as the presenter's
    /// present number `present`, whose arrival counts as a hit or a miss of the budget when
    /// `judged` holds.
    ///
    /// A present numbered zero is not waited for, since its feedback has no number to match.
    pub(crate) fn presented(&mut self, present: u64, target: u64, judged: bool) {
        if present == 0 {
            return;
        }
        if self.waiting.len() == WAITING {
            self.waiting.pop_front();
        }
        self.waiting.push_back(Waiting {
            present,
            target,
            judged,
        });
    }

    /// Returns the refresh interval of the display, from the first source that has one:
    ///
    /// 1. `known`, the interval the platform reported for the display.
    /// 2. The interval the newest feedback reported.
    /// 3. The interval measured from the display times that were not reported late. A
    ///    measurement within 5 % of a multiple of the assumed interval is divided down to one
    ///    interval.
    /// 4. The assumed interval.
    pub(crate) fn interval(&self, known: Option<u64>) -> u64 {
        known
            .or(self.reported)
            .or_else(|| {
                self.refresh
                    .interval
                    .map(|measured| fold(measured, self.nominal))
            })
            .unwrap_or(self.nominal)
    }

    /// Takes the presenter's feedback into the schedule's refreshes and the budget, or the times
    /// the client reported in `reported` when the presenter reports no feedback.
    ///
    /// A reported time answers no frame, so it moves the refreshes and the interval and does not
    /// change the budget. The refreshes follow each display time at the interval that
    /// [`interval`](Self::interval) gives once the time is taken in, where `known` is the
    /// interval the platform reported.
    ///
    /// Each feedback answers the frame presented with its number, and the frames presented
    /// before that one lost their own feedback.
    ///
    /// A judged frame that reached the display after the refresh it was built for counts as a
    /// missed refresh.
    ///
    /// Returns whether the newest frame was shown at an earlier refresh than planned.
    pub(crate) fn read(
        &mut self,
        presenter: &mut dyn Presenter,
        known: Option<u64>,
        reported: &mut Vec<u64>,
        schedule: &mut Schedule,
        budget: &mut Budget,
    ) -> bool {
        if presenter.capabilities().feedback {
            reported.clear();
        } else {
            let reports = reported.drain(..).map(|presented_ns| PresentFeedback {
                image: 0,
                present: 0,
                presented_ns,
                refresh_ns: 0,
            });
            self.feedback.extend(reports);
        }
        presenter.feedback(&mut self.feedback);

        let mut earlier = false;
        let mut batch = std::mem::take(&mut self.feedback);
        for feedback in batch.drain(..) {
            earlier |= self.take(feedback, known, schedule, budget);
        }
        self.feedback = batch;
        earlier
    }

    /// Takes one feedback into the schedule's refreshes and the budget, and reports whether it
    /// shows the newest frame at an earlier refresh than planned.
    fn take(
        &mut self,
        feedback: PresentFeedback,
        known: Option<u64>,
        schedule: &mut Schedule,
        budget: &mut Budget,
    ) -> bool {
        self.reported = (feedback.refresh_ns > 0).then_some(feedback.refresh_ns);
        // Lateness is judged only against refreshes the display times measured.
        let late = self.refresh.interval.is_some() && schedule.reported_late(feedback.presented_ns);
        if known.or(self.reported).is_none() && !late {
            self.refresh.observe(feedback.presented_ns);
        }

        let interval = self.interval(known);
        let answered = answer(&mut self.waiting, feedback.present);
        schedule.interval_ns = interval;
        schedule.anchor(feedback.presented_ns);
        let mut earlier = false;
        if let Some(frame) = answered {
            earlier = schedule.shown(frame.target, feedback.presented_ns);
        }

        match answered {
            // Only a frame built for a refresh can miss it, by reaching the display at the
            // refresh after. A frame that overran its budget is not judged, since its cost
            // already counts in the budget.
            Some(frame) if frame.judged => {
                if feedback.presented_ns > frame.target + interval / 2 {
                    budget.missed(feedback.presented_ns, interval);
                } else {
                    budget.hit();
                }
            }
            _ => {}
        }
        earlier
    }
}

/// Takes the frame in `waiting` that a feedback for present number `present` answers, and drops
/// the frames presented before it.
fn answer(waiting: &mut VecDeque<Waiting>, present: u64) -> Option<Waiting> {
    while waiting.front().is_some_and(|frame| frame.present < present) {
        waiting.pop_front();
    }
    waiting
        .front()
        .is_some_and(|frame| frame.present == present)
        .then(|| waiting.pop_front())
        .flatten()
}

/// Returns `measured_ns`, divided by the multiple of `nominal_ns` it lies within 5 % of.
fn fold(measured_ns: u64, nominal_ns: u64) -> u64 {
    // Frames built every other refresh measure twice the interval, and the driver would then keep
    // building them every other refresh.
    let multiple = (measured_ns + nominal_ns / 2) / nominal_ns.max(1);
    if multiple >= 2 && measured_ns.abs_diff(multiple * nominal_ns) * 20 <= measured_ns {
        measured_ns / multiple
    } else {
        measured_ns
    }
}
