//! Faults injected in place of real acquisitions.

use std::env;

use crate::present::acquisition::Acquisition;

/// The environment variable that queues a fault, as `name` or `name,count`.
pub const VARIABLE: &str = "ICEBLINK_SURFACE_FAULT";

/// Answers to give in place of the next acquisitions, for tests of the recovery paths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FaultInjector {
    queued: Option<(Acquisition, u32)>,
}

impl FaultInjector {
    /// Returns the faults the environment queues.
    pub fn from_environment() -> Self {
        Self {
            queued: parse(env::var(VARIABLE).ok().as_deref()),
        }
    }

    /// Queues `answer` for the next `times` acquisitions, replacing what was queued.
    pub fn inject(&mut self, answer: Acquisition, times: u32) {
        self.queued = (times > 0).then_some((answer, times));
    }

    /// Takes the answer for this acquisition, if one is queued.
    pub fn take(&mut self) -> Option<Acquisition> {
        let (answer, remaining) = self.queued?;
        self.queued = remaining
            .checked_sub(1)
            .and_then(|left| (left > 0).then_some((answer, left)));
        Some(answer)
    }

    /// Returns how many acquisitions still take a queued answer.
    pub fn remaining(&self) -> u32 {
        self.queued.map_or(0, |(_, remaining)| remaining)
    }
}

/// Returns the answer and count `setting` queues, or `None` for a setting that queues nothing.
fn parse(setting: Option<&str>) -> Option<(Acquisition, u32)> {
    let setting = setting?;
    let (name, count) = match setting.split_once(',') {
        Some((name, count)) => (name, count.trim().parse().ok()?),
        None => (setting, 1),
    };
    (count > 0).then_some((named(name.trim())?, count))
}

/// Returns the answer spelled `name`, ignoring case.
fn named(name: &str) -> Option<Acquisition> {
    Acquisition::ALL
        .into_iter()
        .find(|arm| arm.name().eq_ignore_ascii_case(name))
}
