//! A paced driver latches a frame one lead, the budget and the reach, before the refresh it is
//! built for, just after the refresh before it for the compositor's own motion, and immediately
//! for a call on an idle output.

mod common;

use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use iceblink::CompositorClient;
use iceblink::GesturePhase;
use iceblink_core::CompositorImpl;
use iceblink_driver::thread::{ClientHandle, ThreadedCompositor};
use iceblink_driver::{DriverOptions, now_ns};

use crate::common::thread::probe::{Drawn, Probe};
use crate::common::thread::scene::{List, MS, fade};
use crate::common::thread::watched::Watched;

/// The refresh period of the display, long enough that the test thread's timing never decides
/// the result.
const PERIOD_NS: u64 = 40 * MS;

/// The latch budget.
const BUDGET_NS: u64 = 6 * MS;

/// The lead of a display that reports no presents, which keeps the reach of 3 ms the driver
/// assumes before it measured one.
const LEAD_NS: u64 = BUDGET_NS + 3 * MS;

/// Spawns a driver with a fixed budget and opens a list on a display that reports its refreshes
/// only as feedback, once the list's first frame is drawn.
fn open(probe: &Arc<Probe>) -> (ThreadedCompositor, ClientHandle, List) {
    let options = DriverOptions {
        budget_ns: Some(BUDGET_NS),
        ..DriverOptions::default()
    };
    let (driver, mut handle) =
        ThreadedCompositor::spawn_with(Box::new(CompositorImpl::new()), options);
    let list = List::open(&mut handle, Watched::without_deadlines(probe, PERIOD_NS));
    probe.wait_draws(1);
    (driver, handle, list)
}

#[test]
fn an_idle_commit_is_built_at_once() {
    let probe = Probe::new();
    let (driver, mut handle, list) = open(&probe);

    for offset in [30.0, 60.0, 90.0] {
        wait_idle(&handle, &probe);
        let drawn = probe.draws();
        let mut tx = handle.transaction(list.client);
        tx.scroll_to(list.node, [0.0, offset]);
        let before = now_ns();
        handle.commit(tx);
        let after = now_ns();

        let frame = probe.wait_draws(drawn + 1)[drawn].clock;
        assert!(
            frame.now_ns < after + 5 * MS,
            "the frame is built at once, not at the latch one lead before its refresh, \
             {frame:?}, commit between {before} and {after}"
        );
        assert_eq!(
            frame.present_ns % PERIOD_NS,
            0,
            "the frame is built for a refresh of the display, {frame:?}"
        );
        assert!(
            frame.present_ns >= before + LEAD_NS && frame.present_ns < after + LEAD_NS + PERIOD_NS,
            "the frame is built for the refresh it would latch for one lead before, {frame:?}, \
             commit between {before} and {after}"
        );
    }
    assert!(probe.state().violations.is_empty());
    driver.shutdown();
}

#[test]
fn a_client_that_commits_for_every_refresh_keeps_the_latch() {
    let probe = Probe::new();
    let (driver, mut handle, list) = open(&probe);
    wait_idle(&handle, &probe);

    // The client commits just after each refresh, as one that animates on its own does.
    let mut frames = Vec::new();
    for offset in 1..6 {
        let drawn = probe.draws();
        let mut tx = handle.transaction(list.client);
        tx.scroll_to(list.node, [0.0, offset as f32 * 10.0]);
        handle.commit(tx);
        let frame = probe.wait_draws(drawn + 1)[drawn].clock;
        frames.push(frame);
        thread::sleep(Duration::from_nanos(
            (frame.present_ns + MS).saturating_sub(now_ns()),
        ));
    }

    // The first commit found the output idle.
    for frame in &frames[1..] {
        assert!(
            frame.now_ns >= frame.present_ns - LEAD_NS,
            "a frame after one shown at the latest refresh latches one lead before its \
             refresh, {frames:?}"
        );
    }
    assert!(probe.state().violations.is_empty());
    driver.shutdown();
}

#[test]
fn a_frame_that_draws_nothing_leaves_the_output_idle() {
    let probe = Probe::new();
    let (driver, mut handle, list) = open(&probe);

    for offset in [30.0, 60.0, 90.0] {
        wait_idle(&handle, &probe);
        let drawn = probe.draws();
        // A commit that changes nothing latches a frame that draws nothing, which the second
        // `sync` waits for.
        let tx = handle.transaction(list.client);
        handle.commit(tx);
        handle.sync();
        handle.sync();
        assert_eq!(probe.draws(), drawn, "the empty commit draws nothing");

        let mut tx = handle.transaction(list.client);
        tx.scroll_to(list.node, [0.0, offset]);
        handle.commit(tx);
        let after = now_ns();

        let frame = probe.wait_draws(drawn + 1)[drawn].clock;
        assert!(
            frame.now_ns < after + 5 * MS,
            "the frame is built at once, {frame:?}, commit before {after}"
        );
    }
    assert!(probe.state().violations.is_empty());
    driver.shutdown();
}

#[test]
fn a_commit_during_an_animation_keeps_the_latch() {
    let probe = Probe::new();
    let (driver, mut handle, mut list) = open(&probe);
    fade(&mut handle, &mut list, 100 * PERIOD_NS, 0, f32::INFINITY);
    // The animation's first frames start the output's frames at its pace.
    probe.wait_draws(probe.draws() + 3);

    for offset in [30.0, 60.0, 90.0] {
        // The newest frame was shown and the animation's next one is due.
        let shown = probe.state().draws.last().unwrap().clock.present_ns;
        thread::sleep(Duration::from_nanos((shown + MS).saturating_sub(now_ns())));
        let mut tx = handle.transaction(list.client);
        tx.scroll_to(list.node, [0.0, offset]);
        let before = now_ns();
        handle.commit(tx);
        let after = now_ns();

        // The commit joins the animation's frame, which latches just after the refresh before
        // its own.
        let frame = first_frame_after(&probe, after).clock;
        let refresh = frame.present_ns;
        assert_eq!(
            refresh % PERIOD_NS,
            0,
            "the frame is built for a refresh of the display, {frame:?}"
        );
        assert!(
            frame.now_ns >= refresh - PERIOD_NS + PERIOD_NS / 16
                && frame.now_ns < refresh - LEAD_NS,
            "the frame latches just after the refresh before its own, {frame:?}"
        );
        assert!(
            refresh - PERIOD_NS <= before && refresh > after,
            "the frame is built for the first refresh after the commit, {frame:?}, commit \
             between {before} and {after}"
        );
    }
    assert!(probe.state().violations.is_empty());
    driver.shutdown();
}

#[test]
fn a_frame_latched_at_once_shows_the_calls_queued_behind_its_call() {
    let probe = Probe::new();
    let (driver, mut handle, list) = open(&probe);
    wait_idle(&handle, &probe);
    let drawn = probe.draws();

    // The driver thread waits in a renderer task while both commits are queued.
    let (release, wait) = mpsc::channel::<()>();
    handle.with_renderer(
        list.output,
        Box::new(move |_| {
            let _ = wait.recv();
        }),
    );
    for offset in [30.0, 60.0] {
        let mut tx = handle.transaction(list.client);
        tx.scroll_to(list.node, [0.0, offset]);
        handle.commit(tx);
    }
    release.send(()).unwrap();
    handle.sync();
    wait_idle(&handle, &probe);

    let draws = probe.state().draws[drawn..].to_vec();
    assert_eq!(draws.len(), 1, "one frame shows both commits, {draws:?}");
    driver.shutdown();
}

#[test]
fn a_scroll_the_compositor_moves_keeps_the_latch() {
    let probe = Probe::new();
    let (driver, mut handle, list) = open(&probe);

    for _ in 0..3 {
        wait_idle(&handle, &probe);
        // Just after a refresh, a frame latched immediately would be built before the motion's
        // latch.
        let refresh = now_ns().div_ceil(PERIOD_NS) * PERIOD_NS;
        thread::sleep(Duration::from_nanos(
            (refresh + MS).saturating_sub(now_ns()),
        ));
        let drawn = probe.draws();
        list.gesture(&mut handle, 20.0, GesturePhase::Step);

        let frame = probe.wait_draws(drawn + 1)[drawn].clock;
        assert_eq!(
            frame.present_ns,
            refresh + PERIOD_NS,
            "the first frame of the motion is built for the next refresh, {frame:?}"
        );
        assert!(
            frame.now_ns >= refresh + PERIOD_NS / 16,
            "the first frame of the motion does not latch at once, {frame:?}"
        );
    }
    assert!(probe.state().violations.is_empty());
    driver.shutdown();
}

#[test]
fn a_commit_behind_a_frame_shown_early_is_built_for_the_refresh_after_it() {
    let probe = Probe::new();
    let options = DriverOptions {
        budget_ns: Some(BUDGET_NS),
        ..DriverOptions::default()
    };
    let (driver, mut handle) =
        ThreadedCompositor::spawn_with(Box::new(CompositorImpl::new()), options);
    let list = List::open(
        &mut handle,
        Watched::showing_presents(&probe, PERIOD_NS, MS),
    );
    probe.wait_draws(1);

    for offset in [30.0, 60.0, 90.0] {
        wait_idle(&handle, &probe);
        // Less than a budget before a refresh, the idle commit latches immediately for the refresh
        // after it, and its frame reaches the display at the earlier one.
        let refresh = (now_ns() + BUDGET_NS).div_ceil(PERIOD_NS) * PERIOD_NS;
        thread::sleep(Duration::from_nanos(
            (refresh - BUDGET_NS / 2).saturating_sub(now_ns()),
        ));
        let drawn = probe.draws();
        let mut tx = handle.transaction(list.client);
        tx.scroll_to(list.node, [0.0, offset]);
        handle.commit(tx);
        probe.wait_draws(drawn + 1);

        let mut tx = handle.transaction(list.client);
        tx.scroll_to(list.node, [0.0, offset + 10.0]);
        handle.commit(tx);
        let frame = probe.wait_draws(drawn + 2)[drawn + 1].clock;
        assert_eq!(
            frame.present_ns,
            refresh + PERIOD_NS,
            "the commit is built for the refresh after the one the frame before it reached, \
             {frame:?}"
        );
    }
    assert!(probe.state().violations.is_empty());
    driver.shutdown();
}

/// Waits until the driver wants no frame and the refresh after the newest frame has passed.
fn wait_idle(handle: &ClientHandle, probe: &Probe) {
    let patience = Instant::now() + Duration::from_secs(20);
    while handle.stats().next_due_ns.is_some() {
        assert!(Instant::now() < patience, "the driver does not go idle");
        thread::sleep(Duration::from_millis(1));
    }
    let shown = probe
        .state()
        .draws
        .last()
        .map_or(0, |drawn| drawn.clock.present_ns);
    thread::sleep(Duration::from_nanos(
        (shown + PERIOD_NS + MS).saturating_sub(now_ns()),
    ));
}

/// Returns the first frame that starts after `at`.
fn first_frame_after(probe: &Probe, at: u64) -> Drawn {
    probe.wait(|state| {
        state
            .draws
            .iter()
            .find(|drawn| drawn.clock.now_ns > at)
            .cloned()
    })
}
