//! Rules on the scroll motion that wakes a client, and the scroll events it queues.

use iceblink::smallvec::smallvec;
use iceblink::{ClientEvent, OffsetRange, ScrollWatch, WatchCause};
use iceblink::{CompositorEvent, GesturePhase, ScrollId, ScrollPhase, ScrollPhysics};

use crate::conformance::client::Rig;
use crate::conformance::client::stage::ClientStage;

/// The watch causes, wakes and motion steps a client saw after one step of a gesture.
struct Seen {
    /// The watch causes fired.
    causes: Vec<WatchCause>,
    /// How often the client was woken in all.
    woken: usize,
    /// The motion steps the fired watches reported.
    steps: Vec<[f32; 2]>,
}

/// Moves `node` down by 30 pixels per step for `count` steps with an instant touchpad gesture,
/// and returns what the client saw after each step.
fn glide(stage: &mut ClientStage, node: ScrollId, count: usize) -> Vec<Seen> {
    let to = stage.to(node);
    (0..count)
        .map(|index| {
            let phase = if index == 0 {
                GesturePhase::Begin
            } else {
                GesturePhase::Update
            };
            stage.gesture([100.0, 100.0], [0.0, 30.0], phase, to);
            stage.step();

            let fired = watched(stage.events());
            Seen {
                causes: fired.iter().map(|(cause, _)| *cause).collect(),
                woken: stage.woken(),
                steps: fired.iter().map(|(_, step)| *step).collect(),
            }
        })
        .collect()
}

/// Returns the cause and motion step of every watch fired in `events`.
fn watched(events: Vec<ClientEvent>) -> Vec<(WatchCause, [f32; 2])> {
    events
        .into_iter()
        .filter_map(|event| match event {
            ClientEvent::Watched { cause, step, .. } => Some((cause, step)),
            _ => None,
        })
        .collect()
}

/// Returns a vertical range from `low` to `high`, checked `ahead` motion steps ahead.
fn range(low: f32, high: f32, ahead: f32) -> OffsetRange {
    OffsetRange {
        bounds: [[0.0, 0.0], [low, high]],
        ahead,
    }
}

/// Compositor scrolling wakes no client that set no watch.
pub fn scrolling_wakes_no_client_without_a_watch(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let node = stage.list(2000.0, ScrollPhysics::Instant);
    let woken = stage.woken();

    let seen = glide(&mut stage, node, 8);

    let wakes: Vec<usize> = seen.iter().map(|seen| seen.woken - woken).collect();
    assert_eq!(wakes, [0; 8], "no step of the motion wakes the client");
}

/// A queued scroll event of a node is replaced by the node's next one, and a settle is kept.
pub fn a_queued_scroll_event_is_replaced_by_the_next_one_of_its_node(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let node = stage.list(2000.0, ScrollPhysics::Instant);
    let to = stage.to(node);

    stage.gesture([100.0, 100.0], [0.0, 30.0], GesturePhase::Begin, to);
    stage.step();
    for _ in 0..4 {
        stage.gesture([100.0, 100.0], [0.0, 30.0], GesturePhase::Update, to);
        stage.step();
    }
    let moving = scrolls(&stage.events(), node);
    assert_eq!(
        moving,
        [([0.0, 150.0], ScrollPhase::Gesture)],
        "only the newest event of the motion is queued"
    );

    let release = GesturePhase::End {
        velocity: [0.0, 0.0],
    };
    stage.gesture([100.0, 100.0], [0.0, 0.0], release, to);
    stage.steps(4);
    let rest = scrolls(&stage.events(), node);
    assert_eq!(
        rest.last().map(|&(_, phase)| phase),
        Some(ScrollPhase::Settled),
        "the settle is queued, events {rest:?}"
    );
}

/// A range fires once when the shown offset leaves it, and not again until the watch is set
/// again.
pub fn a_range_fires_once_when_the_offset_leaves_it(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let node = stage.list(2000.0, ScrollPhysics::Instant);
    let watch = ScrollWatch {
        ranges: smallvec![range(0.0, 100.0, 0.0)],
        ..ScrollWatch::default()
    };
    stage
        .driver
        .watch_scroll(stage.client, node, Some(watch.clone()));
    let woken = stage.woken();

    let seen = glide(&mut stage, node, 5);

    let causes: Vec<_> = seen.iter().map(|seen| seen.causes.clone()).collect();
    assert_eq!(
        causes,
        [vec![], vec![], vec![], vec![WatchCause::Left(0)], vec![]],
        "the range fires at 120, the first offset past 100"
    );
    assert_eq!(seen[4].woken, woken + 1, "the range wakes the client once");

    stage.driver.watch_scroll(stage.client, node, Some(watch));
    let seen = glide(&mut stage, node, 1);
    assert_eq!(
        seen[0].causes,
        [WatchCause::Left(0)],
        "a watch set again fires again"
    );
}

/// A range checked ahead fires once the offset that many motion steps further leaves it.
pub fn a_range_checked_ahead_fires_before_the_offset_leaves_it(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let node = stage.list(2000.0, ScrollPhysics::Instant);
    let watch = ScrollWatch {
        ranges: smallvec![range(0.0, 100.0, 2.0)],
        ..ScrollWatch::default()
    };
    stage.driver.watch_scroll(stage.client, node, Some(watch));

    let seen = glide(&mut stage, node, 3);

    let causes: Vec<_> = seen.iter().map(|seen| seen.causes.clone()).collect();
    assert_eq!(
        causes,
        [vec![], vec![WatchCause::Left(0)], vec![]],
        "at 60 a motion step of 30 reaches 120 in two steps"
    );
    assert_eq!(
        seen[1].steps,
        [[0.0, 30.0]],
        "the event reports the motion step"
    );
}

/// A stride fires whenever the offset moves past one of its multiples.
pub fn a_stride_fires_at_every_multiple_the_offset_passes(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let node = stage.list(2000.0, ScrollPhysics::Instant);
    let watch = ScrollWatch {
        stride: [0.0, 50.0],
        ..ScrollWatch::default()
    };
    stage.driver.watch_scroll(stage.client, node, Some(watch));

    let seen = glide(&mut stage, node, 4);

    let causes: Vec<_> = seen.iter().map(|seen| seen.causes.clone()).collect();
    assert_eq!(
        causes,
        [
            vec![],
            vec![WatchCause::Crossed],
            vec![],
            vec![WatchCause::Crossed]
        ],
        "the offsets 30, 60, 90 and 120 pass 50 and 100"
    );
}

/// A watch set on a node in motion checks its first event against the node's motion so far.
pub fn a_watch_set_during_motion_knows_the_motion_step(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let node = stage.list(2000.0, ScrollPhysics::Instant);
    glide(&mut stage, node, 2);
    let watch = ScrollWatch {
        ranges: smallvec![range(0.0, 110.0, 1.0)],
        stride: [0.0, 80.0],
        ..ScrollWatch::default()
    };
    stage.driver.watch_scroll(stage.client, node, Some(watch));

    let to = stage.to(node);
    stage.gesture([100.0, 100.0], [0.0, 30.0], GesturePhase::Update, to);
    stage.step();
    let fired = watched(stage.events());
    assert_eq!(
        fired,
        [
            (WatchCause::Left(0), [0.0, 30.0]),
            (WatchCause::Crossed, [0.0, 30.0])
        ],
        "at 90 a motion step of 30 reaches 120 past the range, and the move from 60 passed 80"
    );
}

/// A settle watch fires once when the node comes to rest.
pub fn a_settle_watch_fires_when_the_node_rests(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let node = stage.list(2000.0, ScrollPhysics::Kinetic);
    let watch = ScrollWatch {
        settle: true,
        ..ScrollWatch::default()
    };
    stage.driver.watch_scroll(stage.client, node, Some(watch));
    let woken = stage.woken();

    let to = stage.to(node);
    stage.gesture([100.0, 100.0], [0.0, 40.0], GesturePhase::Step, to);
    let mut settles = 0;
    let mut steps = 0;
    while settles == 0 && steps < 60 {
        stage.step();
        steps += 1;
        settles += stage
            .events()
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    ClientEvent::Watched {
                        cause: WatchCause::Settled,
                        ..
                    }
                )
            })
            .count();
        if settles == 0 {
            assert_eq!(
                stage.woken(),
                woken,
                "the motion wakes nothing before it rests"
            );
        }
    }
    assert_eq!(settles, 1, "the rest fires the watch once");
    assert_eq!(stage.woken(), woken + 1, "the rest wakes the client once");
    assert!(steps > 2, "a kinetic step moves over several frames");
}

/// Returns the offset and phase of every scroll event of `node` in `events`.
fn scrolls(events: &[ClientEvent], node: ScrollId) -> Vec<([f32; 2], ScrollPhase)> {
    events
        .iter()
        .filter_map(|event| match event {
            ClientEvent::Compositor(CompositorEvent::Scroll {
                node: of,
                offset,
                phase,
                ..
            }) if *of == node => Some((*offset, *phase)),
            _ => None,
        })
        .collect()
}
