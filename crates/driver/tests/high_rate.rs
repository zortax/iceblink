//! A paced driver keeps a 144 Hz display at full rate for a client that commits once per refresh
//! after missed refreshes grew its budget, and immediately builds a commit made after a refresh
//! without a new frame, at any phase of the refresh.

mod common;

use std::thread;
use std::time::Duration;

use iceblink::CompositorClient;
use iceblink_driver::now_ns;

use crate::common::display::{Rig, Told, on_time};

/// The refresh period of a 144 Hz display.
const PERIOD_NS: u64 = 6_944_444;

/// How long before a refresh the display takes the images presented to it.
const TAKES_NS: u64 = 1_000_000;

/// How long after a refresh the client commits, as a client that starts its frame when the
/// platform says the previous one was shown.
const PHASE_NS: u64 = 500_000;

/// How many presents the display reports a refresh late, so the driver counts them as missed.
const MISSED: usize = 32;

/// How many refreshes the client commits for after the missed ones.
const STEADY: u64 = 32;

const TOLD: Told = Told {
    period_ns: PERIOD_NS,
    assumed_ns: PERIOD_NS,
    known_ns: None,
    client_reports: false,
    gate_ns: None,
};

/// Reports the first `MISSED` presents a refresh after they reached the display, and the others
/// when they did.
fn late_at_first(index: usize, shown: u64) -> Option<u64> {
    Some(if index < MISSED {
        shown + PERIOD_NS
    } else {
        shown
    })
}

/// Commits `count` changes, `phase_ns` after every `stride`th refresh, building every frame that
/// latches meanwhile, and returns when each commit was made.
fn commit_every(rig: &mut Rig, stride: u64, count: u64, phase_ns: u64) -> Vec<u64> {
    let first = now_ns() / PERIOD_NS + 2;
    let mut commits = Vec::new();
    for index in 0..count {
        let at = (first + index * stride) * PERIOD_NS + phase_ns;
        while rig.driver.stats().next_due_ns.is_some_and(|due| due < at) {
            rig.pump_when_due();
        }
        thread::sleep(Duration::from_nanos(at.saturating_sub(now_ns())));
        commits.push(now_ns());
        rig.commit(index);
    }
    for _ in 0..4 {
        rig.pump_when_due();
    }
    commits
}

/// Returns the commits of the steady run that the display did not show at the first refresh
/// after them, with the refresh it showed them at.
fn late_commits(rig: &Rig, commits: &[u64]) -> Vec<(u64, Option<u64>)> {
    let presented = rig.presented();
    let shown = rig.shown();
    commits[commits.len() - STEADY as usize..]
        .iter()
        .map(|&commit| {
            let first = presented.iter().position(|&present| present >= commit);
            (commit, first.map(|first| shown[first].1))
        })
        .filter(|&(commit, shown)| shown != Some(commit.div_ceil(PERIOD_NS) * PERIOD_NS))
        .collect()
}

#[test]
fn a_client_that_commits_each_refresh_is_shown_at_each_after_missed_refreshes() {
    let mut rig = Rig::new(&TOLD, |_| 0, TAKES_NS, late_at_first);
    let commits = commit_every(&mut rig, 1, MISSED as u64 + STEADY, PHASE_NS);
    let late = late_commits(&rig, &commits);
    assert!(
        late.is_empty(),
        "every commit is shown at the refresh after it, late: {late:?}"
    );
}

#[test]
fn commits_after_a_refresh_without_a_frame_are_presented_at_once_at_any_phase() {
    // Up to 2 ms before the refresh, so each frame reaches the refresh after its commit.
    for phase in (1..11).map(|step| step * PERIOD_NS / 14) {
        let mut rig = Rig::new(&TOLD, |_| 0, TAKES_NS, on_time);
        let commits = commit_every(&mut rig, 2, 12, phase);
        let presented = rig.presented();
        let shown: Vec<u64> = rig.shown().iter().map(|&(_, shown)| shown).collect();
        assert!(
            shown.windows(2).all(|pair| pair[1] > pair[0]),
            "at {phase} ns after a refresh, no two frames are shown at one refresh: {shown:?}"
        );
        // The first commits measure the frames and the refreshes.
        let waited: Vec<u64> = commits[4..]
            .iter()
            .filter_map(|&commit| {
                let present = presented.iter().find(|&&present| present >= commit)?;
                (present - commit > 1_000_000).then_some(present - commit)
            })
            .collect();
        assert!(
            waited.is_empty(),
            "at {phase} ns after a refresh, every commit is presented at once, waits: {waited:?}"
        );
    }
}
