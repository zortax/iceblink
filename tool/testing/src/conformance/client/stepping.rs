//! Rules on stepped drivers: client calls build at the next step, compositor motion advances once
//! per interval, and each step's results are published before the next step builds.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use iceblink::smallvec::smallvec;
use iceblink::{
    Animation, AnimationStart, AnimationTarget, BlendMode, ChunkKey, Direction, Easing, EffectNode,
    FillMode, FrameId, Keyframe, Keyframes, OrderEntry, OrderNode, Timing,
};
use iceblink::{ClientEvent, CommitId};

use crate::conformance::client::Rig;
use crate::conformance::client::seen::Seen;
use crate::conformance::client::stage::{ClientStage, STEP_NS, quads};

/// A commit queued after a frame at one instant is built by a step at the same instant.
pub fn commits_at_one_instant_build_a_frame_each(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let before = stage.frames();

    for index in 0..3 {
        commit_square(&mut stage, index);
        if index == 0 {
            stage.step();
        } else {
            stage.step_again();
        }
    }

    assert_eq!(
        stage.frames(),
        before + 3,
        "each commit is built by the next step, at the same instant too"
    );
}

/// A running animation advances once per interval of the step clocks, however often the driver
/// is stepped at one instant.
pub fn motion_advances_once_per_interval(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let mut tx = stage.tx();
    let effect = tx.create_effect(EffectNode {
        clip: None,
        opacity: 0.5,
        blend: BlendMode::NORMAL,
        filters: Arc::from([]),
        isolate: false,
    });
    let group = tx.create_order(OrderNode {
        effect: Some(effect),
    });
    let key = ChunkKey::new(1, 0);
    quads(&mut tx, key, stage.space, &[[20.0, 20.0, 100.0, 100.0]]);
    tx.splice(group, 0, 0, &[OrderEntry::Chunk(key)]);
    stage.push(&mut tx, OrderEntry::Node(group));
    let linear = |offset, value| Keyframe {
        offset,
        value,
        easing: Easing::Linear,
    };
    tx.animate(Animation {
        target: AnimationTarget::Opacity(effect),
        keyframes: Keyframes::Scalar(smallvec![linear(0.0, 0.2), linear(1.0, 0.8)]),
        timing: Timing {
            duration_ns: 60 * STEP_NS,
            delay_ns: 0,
            iterations: f32::INFINITY,
            direction: Direction::Alternate,
            fill: FillMode::None,
            easing: Easing::Linear,
        },
        start: AnimationStart::Next,
    });
    stage.driver.commit(tx);
    stage.steps(2);

    let before = stage.frames();
    for _ in 0..3 {
        stage.step_again();
    }
    let again = stage.frames();
    stage.step();

    assert_eq!(
        again, before,
        "steps at one instant do not advance the animation"
    );
    assert_eq!(
        stage.frames(),
        before + 1,
        "the next interval advances it once"
    );
}

/// A stepped driver builds each frame at the clock it was stepped with.
pub fn a_stepped_frame_is_built_at_its_step_clock(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let before = stage.frames();
    let mut clocks = Vec::new();
    for index in 0..3 {
        commit_square(&mut stage, index);
        clocks.push(stage.clock());
        stage.step();
    }

    let log = stage.log();
    let built: Vec<_> = log.draws[before..]
        .iter()
        .map(|frame| frame.clock)
        .collect();
    assert_eq!(built, clocks, "each frame is built at its step's clock");
}

/// A client that queues commits and steps without waiting for them sees each step's commit
/// answer and frame report, in order, before the driver builds the next step's frame.
pub fn a_flooding_client_sees_each_frame_before_the_next(rig: &Rig<'_>) {
    const STEPS: usize = 3;
    let mut stage = ClientStage::new(rig);
    stage.driver.watch_frames(stage.client, stage.output, true);
    let seen = Arc::new(Seen::default());
    let waited = Arc::clone(&seen);
    let in_time: Arc<Mutex<Vec<bool>>> = Arc::default();
    let noted = Arc::clone(&in_time);
    stage.driver.observe_frames(
        stage.output,
        Some(Box::new(move |_| {
            let mut noted = noted.lock().unwrap();
            let count = noted.len();
            noted.push(waited.wait_for(count));
        })),
    );
    let before = stage.frames();

    let mut events = Vec::new();
    let mut commits = Vec::new();
    for index in 0..STEPS {
        commits.push(commit_square(&mut stage, index as u64));
        let clock = stage.clock();
        stage.advance();
        stage.driver.step(clock);
        poll(&mut stage, &mut events, &seen);
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    while drawn(&events) < STEPS && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
        poll(&mut stage, &mut events, &seen);
    }
    stage.driver.observe_frames(stage.output, None);
    stage.driver.sync();

    assert_eq!(
        *in_time.lock().unwrap(),
        [true; STEPS],
        "the client sees each frame reported before the next one is built"
    );
    let log = stage.log();
    let expected: Vec<_> = commits
        .iter()
        .zip(&log.draws[before..])
        .flat_map(|(commit, frame)| [Answer::Committed(*commit), Answer::Drawn(frame.draw.frame)])
        .collect();
    let answers: Vec<_> = events.iter().filter_map(Answer::of).collect();
    assert_eq!(
        answers, expected,
        "each commit is answered and each frame reported, in order"
    );
}

/// Commits chunk `index + 1` with one 8 by 8 square, `index` squares from the left, and returns
/// the commit.
fn commit_square(stage: &mut ClientStage, index: u64) -> CommitId {
    let mut tx = stage.tx();
    let key = ChunkKey::new(1 + index, 0);
    quads(
        &mut tx,
        key,
        stage.space,
        &[[10.0 * index as f32, 0.0, 8.0, 8.0]],
    );
    stage.push(&mut tx, OrderEntry::Chunk(key));
    stage.driver.commit(tx)
}

/// A commit answer or a frame report.
#[derive(Debug, PartialEq)]
enum Answer {
    Committed(CommitId),
    Drawn(FrameId),
}

impl Answer {
    fn of(event: &ClientEvent) -> Option<Self> {
        match event {
            ClientEvent::Committed { commit, .. } => Some(Self::Committed(*commit)),
            ClientEvent::Drawn { frame, .. } => Some(Self::Drawn(*frame)),
            _ => None,
        }
    }
}

/// Drains the client's events into `events` and records the frames reported so far in `seen`.
fn poll(stage: &mut ClientStage, events: &mut Vec<ClientEvent>, seen: &Seen) {
    stage.driver.drain_events(stage.client, events);
    seen.set(drawn(events));
}

fn drawn(events: &[ClientEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, ClientEvent::Drawn { .. }))
        .count()
}

/// A client that steps without waiting reads the snapshot as the state after its newest step once
/// the snapshot counts every step it made.
pub fn a_snapshot_is_current_once_it_counts_every_step(rig: &Rig<'_>) {
    const STEPS: u64 = 4;
    let mut stage = ClientStage::new(rig);
    let before = stage.driver.stats();

    for index in 0..STEPS {
        commit_square(&mut stage, index);
        let clock = stage.clock();
        stage.advance();
        stage.driver.step(clock);
        assert!(
            stage.driver.stats().steps <= before.steps + index + 1,
            "a snapshot counts no step that was not made"
        );
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut stats = stage.driver.stats();
    while stats.steps < before.steps + STEPS && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
        stats = stage.driver.stats();
    }

    assert_eq!(
        stats.steps,
        before.steps + STEPS,
        "the snapshot catches up with every step"
    );
    assert_eq!(
        (stats.frames_built, stats.next_due_ns),
        (before.frames_built + STEPS, None),
        "the snapshot that counts every step shows each step's frame and the driver idle"
    );
}
