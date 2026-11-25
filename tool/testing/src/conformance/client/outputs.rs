//! Rules on outputs and their renderers: failures, renderer work, releases, frame observers,
//! watches of presents and presenter state, and the display time.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use iceblink::{ChunkKey, FrameId, FrameOutcome, GpuUnavailable, OrderEntry, SkipReason};
use iceblink::{ClientEvent, OutputConfig, OutputWatch, Renderer};

use crate::conformance::client::Rig;
use crate::conformance::client::recorder::{Recorder, Release, read};
use crate::conformance::client::stage::{ClientStage, STEP_NS, quads};

/// Commits a quad that changes the output, under chunk `index`.
fn change(stage: &mut ClientStage, index: u64) {
    let mut tx = stage.tx();
    let key = ChunkKey::new(1 + index, 0);
    let at = 10.0 * index as f32;
    quads(&mut tx, key, stage.space, &[[at, 0.0, 8.0, 8.0]]);
    stage.push(&mut tx, OrderEntry::Chunk(key));
    stage.driver.commit(tx);
}

/// Has the output's renderer answer its next frame with `outcome`.
fn answer_next(stage: &mut ClientStage, outcome: FrameOutcome) {
    stage.driver.with_renderer(
        stage.output,
        Box::new(move |renderer: &mut dyn Renderer| {
            let recorder = renderer
                .as_any_mut()
                .and_then(|any| any.downcast_mut::<Recorder>())
                .expect("the output's renderer is the stage's recorder");
            recorder.push_outcome(outcome);
        }),
    );
}

/// A renderer factory that fails leaves an output that draws nothing, reports it failed, and
/// wakes every client with an `OutputFailed` event for it.
pub fn a_failed_renderer_reports_its_output(rig: &Rig<'_>) {
    let mut driver = (rig.client)((rig.compositor)());
    let (client, _) = driver.register_client();
    let wakes = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&wakes);
    driver.set_waker(
        client,
        Some(Arc::new(move || {
            counted.fetch_add(1, Ordering::Relaxed);
        })),
    );
    let error = GpuUnavailable::default().rejected("the only adapter", "no surface");
    let failure = error.clone();
    let config = OutputConfig {
        size: [64, 64],
        scale: 1.0,
        opaque: true,
    };
    let output = driver.create_output(config, Box::new(move || Err(failure)));
    driver.sync();

    let mut events = Vec::new();
    driver.drain_events(client, &mut events);
    assert_eq!(
        events,
        [ClientEvent::OutputFailed { output, error }],
        "the failure names the output"
    );
    assert_eq!(
        wakes.load(Ordering::Relaxed),
        1,
        "the failure wakes the client"
    );
    let status = driver.output_status(output);
    assert!(
        status.is_some_and(|status| status.failed && status.frames_built == 0),
        "the output's snapshot reports the failure, {status:?}"
    );
}

/// Work on a renderer runs between the frames queued around it, on the output's renderer.
pub fn renderer_tasks_run_between_the_frames_around_them(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    change(&mut stage, 0);
    stage.step();
    let before = stage.frames();

    let seen = Arc::new(Mutex::new(None));
    let noted = Arc::clone(&seen);
    let log = Arc::clone(&stage.log);
    change(&mut stage, 1);
    stage.driver.with_renderer(
        stage.output,
        Box::new(move |renderer: &mut dyn Renderer| {
            let recorder = renderer
                .as_any()
                .and_then(|any| any.downcast_ref::<Recorder>())
                .is_some();
            *noted.lock().unwrap() = Some((recorder, read(&log).draws.len()));
        }),
    );
    stage.step();

    assert_eq!(
        *seen.lock().unwrap(),
        Some((true, before)),
        "the task sees the output's renderer after the frames before it and before the next"
    );
    assert_eq!(stage.frames(), before + 1, "the step after the task draws");
}

/// Releases of renderer resources are passed to the output's renderer in the order they were
/// queued.
pub fn releases_reach_the_renderer(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    stage.driver.release_idle_resources(stage.output);
    stage.driver.release_cached_targets(stage.output);
    stage.driver.sync();

    assert_eq!(
        read(&stage.log).released,
        [Release::IdleResources, Release::CachedTargets],
        "both releases reach the renderer in order"
    );
}

/// A frame observer sees every frame built for its output before the frame is drawn, until it is
/// removed.
pub fn a_frame_observer_sees_each_frame_before_it_is_drawn(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let seen: Arc<Mutex<Vec<(FrameId, usize)>>> = Arc::default();
    let noted = Arc::clone(&seen);
    let log = Arc::clone(&stage.log);
    stage.driver.observe_frames(
        stage.output,
        Some(Box::new(move |frame| {
            let drawn = read(&log).draws.len();
            noted.lock().unwrap().push((frame.id, drawn));
        })),
    );
    let before = stage.frames();
    for index in 0..3 {
        change(&mut stage, index);
        stage.step();
    }
    stage.driver.observe_frames(stage.output, None);
    change(&mut stage, 3);
    stage.step();

    let log = stage.log();
    let expected: Vec<(FrameId, usize)> = log.draws[before..before + 3]
        .iter()
        .enumerate()
        .map(|(index, frame)| (frame.draw.frame, before + index))
        .collect();
    assert_eq!(
        *seen.lock().unwrap(),
        expected,
        "each frame is observed once, before it is drawn, and none after the removal"
    );
}

/// A shown watch fires once, on the next frame the output presents.
pub fn a_shown_watch_fires_once_on_the_next_present(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let watch = OutputWatch {
        shown: true,
        ..OutputWatch::default()
    };
    stage.driver.watch_output(stage.client, stage.output, watch);
    let woken = stage.woken();
    answer_next(&mut stage, FrameOutcome::Skipped(SkipReason::Undamaged));
    change(&mut stage, 0);
    stage.step();
    let skipped = shown(&stage.events());
    change(&mut stage, 1);
    stage.step();
    let presented = stage.log().draws.last().map(|frame| frame.draw.frame);
    let first = shown(&stage.events());
    change(&mut stage, 2);
    stage.step();
    let later = shown(&stage.events());

    assert!(skipped.is_empty(), "a skipped frame shows nothing");
    assert_eq!(
        first.first().copied(),
        presented,
        "the next presented frame is shown"
    );
    assert_eq!(first.len(), 1, "the watch fires once");
    assert!(later.is_empty(), "the watch fires only once");
    assert_eq!(stage.woken(), woken + 1, "the present wakes the client");
}

/// A presenter watch reports when frames start and stop timing out, and when the output is hidden
/// and shown again, and the output's snapshot holds the same state.
pub fn presenter_changes_follow_starved_and_occluded_frames(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let watch = OutputWatch {
        presenter: true,
        ..OutputWatch::default()
    };
    stage.driver.watch_output(stage.client, stage.output, watch);

    answer_next(&mut stage, FrameOutcome::Skipped(SkipReason::Timeout));
    change(&mut stage, 0);
    stage.step();
    let starved = stage
        .driver
        .output_status(stage.output)
        .is_some_and(|status| status.starved);
    change(&mut stage, 1);
    stage.step();
    answer_next(&mut stage, FrameOutcome::Skipped(SkipReason::Occluded));
    change(&mut stage, 2);
    stage.step();
    change(&mut stage, 3);
    stage.driver.request_frame(stage.output);
    stage.step();

    assert!(starved, "the snapshot reports a starved presenter");
    assert_eq!(
        changes(&stage.events()),
        [(true, false), (false, false), (false, true), (false, false)],
        "every change of the presenter state is reported"
    );
}

/// The display time of the next frame follows the schedule.
pub fn the_next_display_time_follows_the_schedule(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    stage.steps(2);
    let idle = stage.driver.stats();
    change(&mut stage, 0);
    stage.driver.sync();
    let wanted = stage.driver.stats();

    assert_eq!(
        idle.next_display_ns, None,
        "an idle driver predicts no display time"
    );
    let due = wanted.next_due_ns.expect("the commit wants a frame");
    assert_eq!(
        wanted.next_display_ns,
        Some(due + STEP_NS),
        "a stepped frame is predicted one interval after it is due"
    );
}

/// Returns the frame of every `Shown` event in `events`.
fn shown(events: &[ClientEvent]) -> Vec<FrameId> {
    events
        .iter()
        .filter_map(|event| match event {
            ClientEvent::Shown { frame, .. } => Some(*frame),
            _ => None,
        })
        .collect()
}

/// Returns whether the presenter is starved and occluded after every change in `events`.
fn changes(events: &[ClientEvent]) -> Vec<(bool, bool)> {
    events
        .iter()
        .filter_map(|event| match event {
            ClientEvent::PresenterChanged {
                starved, occluded, ..
            } => Some((*starved, *occluded)),
            _ => None,
        })
        .collect()
}

/// A frame whose image could not be acquired is reported to frame watchers, and the next frame
/// draws what it would have drawn.
pub fn a_failed_acquisition_is_reported_and_retried(rig: &Rig<'_>) {
    let fail = Arc::new(AtomicBool::new(false));
    let mut stage = ClientStage::flaky(rig, Arc::clone(&fail));
    stage.driver.watch_frames(stage.client, stage.output, true);
    stage.steps(2);
    stage.events();
    let before = stage.frames();

    fail.store(true, Ordering::Relaxed);
    change(&mut stage, 0);
    stage.step();
    let skipped = stage.events();
    let after_skip = stage.frames();
    stage.step();
    let retried = stage.events();

    let outcomes = |events: &[ClientEvent]| -> Vec<FrameOutcome> {
        events
            .iter()
            .filter_map(|event| match event {
                ClientEvent::Drawn { outcome, .. } => Some(*outcome),
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        outcomes(&skipped),
        [FrameOutcome::Skipped(SkipReason::Timeout)],
        "the failed acquisition is reported"
    );
    assert_eq!(after_skip, before, "nothing was drawn for it");
    assert!(
        matches!(outcomes(&retried)[..], [FrameOutcome::Presented(_)]),
        "the next step presents a frame, events {retried:?}"
    );
    let log = stage.log();
    assert!(
        log.draws.last().is_some_and(|frame| frame.quads >= 1),
        "the retry draws the change the skipped frame left"
    );
}
