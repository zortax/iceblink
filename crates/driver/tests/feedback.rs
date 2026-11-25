//! A paced driver measures the display's refreshes from the times its presents reached the
//! display, as its presenter or its client reports them, and grows its budget when frames miss the
//! refresh they were built for. Reports that come late or get lost move neither.

mod common;

use std::thread;
use std::time::Duration;

use iceblink::CompositorClient;
use iceblink_driver::now_ns;

use crate::common::display::{Cost, Report, Rig, Told, on_time};

/// The display's refresh period, long enough that the test thread's timing never decides the
/// result.
const PERIOD_NS: u64 = 40_000_000;

/// How long before a refresh a slow display takes the images presented to it.
const TAKES_NS: u64 = 6_000_000;

const FRAMES: usize = 40;

/// An assumed interval the display does not have, and of which it is no multiple, and no
/// reported one.
const UNTOLD: Told = Told {
    period_ns: PERIOD_NS,
    assumed_ns: PERIOD_NS * 7 / 10,
    known_ns: None,
    client_reports: false,
    gate_ns: None,
};

/// Runs `FRAMES` frames that each commit a change, pumping the driver whenever a frame latches,
/// on a display that takes images `takes_ns` before a refresh and reports its presents with
/// `report`. Returns what the display showed and the budget of a frame wanted after the last.
fn run(takes_ns: u64, report: Report) -> (Vec<(u64, u64)>, u64) {
    run_told(&UNTOLD, FRAMES, |_| 0, takes_ns, report)
}

/// Runs as [`run`] does for `frames` frames, with the driver told `told` of the display and frames
/// that take `cost` to draw.
fn run_told(
    told: &Told,
    frames: usize,
    cost: Cost,
    takes_ns: u64,
    report: Report,
) -> (Vec<(u64, u64)>, u64) {
    let mut rig = Rig::new(told, cost, takes_ns, report);
    for index in 0..frames as u64 {
        rig.commit(index);
        assert!(rig.pump_when_due(), "the commit wants a frame");
    }
    rig.commit(frames as u64);
    let stats = rig.driver.stats();
    let budget = stats.next_display_ns.unwrap_or(0) - stats.next_due_ns.unwrap_or(0);
    (rig.shown(), budget)
}

#[test]
fn frames_follow_the_measured_refreshes() {
    let (shown, _) = run(TAKES_NS, on_time);
    let late: Vec<_> = shown[FRAMES / 2..]
        .iter()
        .filter(|(planned, _)| planned % PERIOD_NS != 0)
        .collect();
    assert!(
        late.is_empty(),
        "every later frame is built for a refresh of the display, off the grid: {late:?}"
    );
}

#[test]
fn frames_follow_the_refreshes_the_client_reports() {
    let told = Told {
        client_reports: true,
        ..UNTOLD
    };
    let (shown, _) = run_told(&told, FRAMES, |_| 0, TAKES_NS, on_time);
    let late: Vec<_> = shown[FRAMES / 2..]
        .iter()
        .filter(|(planned, _)| planned % PERIOD_NS != 0)
        .collect();
    assert!(
        late.is_empty(),
        "every later frame is built for a refresh the client reported, off the grid: {late:?}"
    );
}

/// The display's true period, which the driver is told it assumes.
const TOLD: Told = Told {
    period_ns: PERIOD_NS,
    assumed_ns: PERIOD_NS,
    known_ns: None,
    client_reports: false,
    gate_ns: None,
};

/// How long before a refresh a fast display takes the images presented to it.
const QUICK_NS: u64 = 1_000_000;

/// Returns the frames of `shown` that came more than one refresh after the frame before.
fn skips(shown: &[(u64, u64)]) -> Vec<(u64, u64)> {
    shown
        .windows(2)
        .filter(|pair| pair[1].1 > pair[0].1 + PERIOD_NS)
        .map(|pair| pair[1])
        .collect()
}

#[test]
fn the_first_frames_skip_no_refresh_before_any_feedback() {
    let (shown, _) = run_told(&TOLD, 6, |_| 0, QUICK_NS, |_, _| None);
    assert!(
        skips(&shown).is_empty(),
        "every frame reaches the refresh after the one before: {shown:?}"
    );
}

#[test]
fn the_first_feedback_moves_the_frame_already_due() {
    let (shown, _) = run_told(&TOLD, 6, |_| 0, QUICK_NS, on_time);
    assert!(
        skips(&shown).is_empty(),
        "every frame reaches the refresh after the one before: {shown:?}"
    );
}

#[test]
fn a_late_frame_is_followed_by_frames_at_full_rate() {
    let (shown, _) = run_told(
        &TOLD,
        FRAMES,
        |index| {
            if index == FRAMES / 2 {
                PERIOD_NS * 3 / 2
            } else {
                0
            }
        },
        QUICK_NS,
        on_time,
    );
    let after = &shown[FRAMES / 2 + 2..];
    assert!(
        skips(after).is_empty(),
        "the frames after the late one reach every refresh: {after:?}"
    );
}

#[test]
fn a_held_frame_reaches_its_refresh_once_the_platform_lets_it() {
    // The platform holds every frame until 3 ms before the refresh after the previous one, past
    // the latch one budget before it.
    let told = Told {
        gate_ns: Some(PERIOD_NS - 3_000_000),
        ..TOLD
    };
    let (shown, _) = run_told(&told, FRAMES, |_| 0, QUICK_NS, on_time);
    let later = &shown[4..];
    assert!(
        skips(later).is_empty(),
        "every later frame reaches the refresh after the one before: {later:?}"
    );
}

#[test]
fn a_client_that_commits_after_each_frame_is_shown_returns_to_full_rate() {
    let mut rig = Rig::new(&TOLD, |_| 0, QUICK_NS, on_time);
    for index in 0..FRAMES as u64 {
        // The client composes once the frame before reached the display, and one compose takes
        // longer than a refresh.
        let shown = rig.shown().last().map_or(0, |&(_, shown)| shown);
        let compose = if index == FRAMES as u64 / 2 {
            PERIOD_NS + PERIOD_NS / 4
        } else {
            2_000_000
        };
        thread::sleep(Duration::from_nanos(
            (shown + compose).saturating_sub(now_ns()),
        ));
        rig.commit(index);
        while rig.shown().len() as u64 <= index && rig.pump_when_due() {}
    }
    let shown = rig.shown();
    let after = &shown[FRAMES / 2 + 2..];
    assert!(
        skips(after).is_empty(),
        "the frames after the late compose reach every refresh: {after:?}"
    );
}

#[test]
fn frames_on_a_display_that_holds_two_presents_are_built_for_the_refresh_that_shows_them() {
    let (shown, _) = run_told(&TOLD, FRAMES, |_| 0, PERIOD_NS + QUICK_NS, on_time);
    let missed: Vec<_> = shown[FRAMES / 2..]
        .iter()
        .filter(|(planned, shown)| planned != shown)
        .collect();
    assert!(
        missed.is_empty(),
        "every later frame is shown at the refresh it was built for, missed: {missed:?}"
    );
}

#[test]
fn missed_refreshes_grow_the_budget() {
    let (shown, _) = run(TAKES_NS, on_time);
    let missed: Vec<_> = shown[FRAMES / 2..]
        .iter()
        .filter(|(planned, shown)| planned != shown)
        .collect();
    assert!(
        missed.is_empty(),
        "every later frame reaches the refresh it was built for, missed: {missed:?}"
    );
}

#[test]
fn late_reports_keep_the_frames_on_the_refreshes() {
    let (shown, _) = run(TAKES_NS, |index, shown| {
        let late = index >= FRAMES / 4 && index % 3 == 0;
        Some(shown + if late { 5_000_000 } else { 0 })
    });
    let late: Vec<_> = shown[FRAMES / 2..]
        .iter()
        .filter(|(planned, _)| planned % PERIOD_NS != 0)
        .collect();
    assert!(
        late.is_empty(),
        "a report that came late moves no display time off the grid: {late:?}"
    );
}

#[test]
fn a_lost_report_counts_no_frame_as_missed() {
    let (_, budget) = run(0, |index, shown| (index != FRAMES / 2).then_some(shown));
    assert_eq!(
        budget, 4_000_000,
        "the frames after a lost report are answered by their own reports, so the budget keeps \
         its floor"
    );
}

#[test]
fn a_reported_refresh_is_kept_when_the_assumed_one_divides_it() {
    let told = Told {
        period_ns: PERIOD_NS,
        assumed_ns: PERIOD_NS / 2,
        known_ns: Some(PERIOD_NS),
        client_reports: false,
        gate_ns: None,
    };
    let (shown, _) = run_told(&told, FRAMES, |_| 0, 0, on_time);
    let off: Vec<_> = shown[FRAMES / 2..]
        .iter()
        .filter(|(planned, _)| planned % PERIOD_NS != 0)
        .collect();
    assert!(
        off.is_empty(),
        "a display reported at half the assumed rate keeps its refreshes: {off:?}"
    );
}

#[test]
fn one_heavy_frame_leaves_the_budget_to_the_light_frames() {
    let (_, budget) = run_told(
        &UNTOLD,
        FRAMES,
        |index| if index == FRAMES - 4 { 12_000_000 } else { 0 },
        0,
        on_time,
    );
    // The heavy frame misses its refresh, which its cost explains.
    assert_eq!(
        budget, 4_000_000,
        "four light frames after a heavy one keep the budget at the floor"
    );
}

#[test]
fn sustained_heavy_frames_grow_the_budget() {
    let (_, budget) = run_told(
        &UNTOLD,
        FRAMES,
        |index| if index >= FRAMES - 8 { 8_000_000 } else { 0 },
        0,
        on_time,
    );
    assert!(
        budget >= 10_000_000,
        "eight frames of 8 ms grow the budget to their cost and a quarter: {budget}"
    );
}

#[test]
fn the_first_frame_leaves_the_budget_alone() {
    let (_, budget) = run_told(
        &UNTOLD,
        6,
        |index| {
            if index == 0 || index == 2 {
                12_000_000
            } else {
                0
            }
        },
        0,
        on_time,
    );
    assert!(
        budget <= 5_000_000,
        "an expensive first frame and one heavy frame after it keep the budget at the floor, \
         and at most one miss above it: {budget}"
    );
}
