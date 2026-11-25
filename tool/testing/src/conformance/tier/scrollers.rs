//! Scroll fixtures: behaviours, ports onto rows of content, and frames drawn across a motion.

use std::sync::Arc;

use iceblink::{
    Anchor, ChunkKey, CompositorEvent, GesturePhase, Matrix4, OrderEntry, ScrollBehaviour,
    ScrollId, ScrollNode, ScrollPhysics, ScrollRouting, SnapKind, SnapPoint, Sources, Transaction,
    TransformId, TransformNode,
};

use crate::conformance::tier::stage::{Factory, Stage, props, quads};

/// The share of a fling's velocity left after one second.
pub(super) const DECAY: f64 = 0.002;

/// A behaviour that scrolls vertically, bouncing and chaining the gestures of `bounce` and
/// `chain`.
pub(super) fn vertical(physics: ScrollPhysics, bounce: Sources, chain: Sources) -> ScrollBehaviour {
    ScrollBehaviour {
        physics,
        snap: Arc::from([]),
        snap_kind: SnapKind::Mandatory,
        bounce,
        user_scrollable: [false, true],
        chain,
    }
}

/// A scroll node and the transform node that applies its offset to the content.
pub(super) struct Scroller {
    pub(super) node: ScrollId,
    pub(super) content: TransformId,
}

/// Creates a scroll node over `port` in `space` with content `height` tall, and the transform
/// node that applies it.
pub(super) fn scroller(
    tx: &mut Transaction,
    space: TransformId,
    port: [f32; 4],
    height: f32,
    behaviour: Option<ScrollBehaviour>,
) -> Scroller {
    let node = tx.create_scroll(ScrollNode {
        space,
        port,
        content: [port[2], height],
        offset: [0.0; 2],
        behaviour,
    });
    let content = tx.create_transform(TransformNode {
        parent: Some(space),
        local: Matrix4::IDENTITY,
        scroll: Some(node),
        anchor: Anchor::Scrolls,
    });
    Scroller { node, content }
}

/// Rows 20 tall and 200 wide from the top of the content down to `height`.
fn rows(height: f32) -> Vec<[f32; 4]> {
    (0..(height / 20.0) as usize)
        .map(|row| [0.0, row as f32 * 20.0, 200.0, 20.0])
        .collect()
}

/// A stage whose whole output is a port onto content 2000 tall.
pub(super) fn list(factory: Factory<'_>, behaviour: Option<ScrollBehaviour>) -> (Stage, ScrollId) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    let list = scroller(
        &mut tx,
        stage.space,
        [0.0, 0.0, 200.0, 200.0],
        2000.0,
        behaviour,
    );
    quads(
        &mut tx,
        ChunkKey::new(1, 0),
        props(list.content),
        &rows(2000.0),
    );
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(1, 0)));
    stage.commit(tx);
    stage.draw_at(0);
    (stage, list.node)
}

/// A stage with an outer port over the whole output onto content 1000 tall, and in its content
/// an inner port over the top 100 pixels onto content 400 tall.
pub(super) fn nested(factory: Factory<'_>, inner: ScrollBehaviour) -> (Stage, ScrollId, ScrollId) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    let outer_behaviour = vertical(ScrollPhysics::Instant, Sources::NONE, Sources::NONE);
    let outer = scroller(
        &mut tx,
        stage.space,
        [0.0, 0.0, 200.0, 200.0],
        1000.0,
        Some(outer_behaviour),
    );
    let inner = scroller(
        &mut tx,
        outer.content,
        [0.0, 0.0, 200.0, 100.0],
        400.0,
        Some(inner),
    );
    quads(
        &mut tx,
        ChunkKey::new(1, 0),
        props(outer.content),
        &rows(1000.0),
    );
    quads(
        &mut tx,
        ChunkKey::new(2, 0),
        props(inner.content),
        &rows(400.0),
    );
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(1, 0)));
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(2, 0)));
    stage.commit(tx);
    stage.draw_at(0);
    (stage, outer.node, inner.node)
}

/// Draws frames from `from_ns` every refresh interval up to `to_ns`, and returns every event.
pub(super) fn frames(stage: &mut Stage, from_ns: u64, to_ns: u64) -> Vec<CompositorEvent> {
    per_frame(stage, from_ns, to_ns)
        .into_iter()
        .flatten()
        .collect()
}

/// Draws frames from `from_ns` every refresh interval up to `to_ns`, and returns the events of
/// each frame.
pub(super) fn per_frame(stage: &mut Stage, from_ns: u64, to_ns: u64) -> Vec<Vec<CompositorEvent>> {
    let mut frames = Vec::new();
    let mut at = from_ns;
    while at <= to_ns {
        stage.draw_at(at);
        frames.push(stage.events());
        at += stage.interval_ns;
    }
    frames
}

/// Returns where a fling released at `velocity` pixels per second ends, from its start, and how
/// many seconds after its release it ends, at a refresh interval of `interval_ns`.
pub(super) fn fling_end(velocity: f64, interval_ns: u64) -> (f64, f64) {
    let rest = 1e9 / interval_ns as f64;
    let seconds = (rest / velocity).ln() / DECAY.ln();
    ((velocity - rest) / -DECAY.ln(), seconds)
}

/// Begins a touchpad gesture with a delta of 10 down and releases it at `velocity` pixels per
/// second down, both at time zero.
pub(super) fn fling(stage: &mut Stage, velocity: f32) {
    stage.gesture([100.0, 100.0], [0.0, 10.0], GesturePhase::Begin, 0);
    stage.gesture(
        [100.0, 100.0],
        [0.0, 0.0],
        GesturePhase::End {
            velocity: [0.0, velocity],
        },
        0,
    );
}

/// Snap points at the vertical offsets `offsets`.
pub(super) fn snaps(offsets: &[f32]) -> Arc<[SnapPoint]> {
    offsets
        .iter()
        .map(|&y| SnapPoint {
            offset: [f32::NAN, y],
        })
        .collect()
}

/// Returns the node `routing` hands the gesture to, if any.
pub(super) fn handled(routing: ScrollRouting) -> Option<ScrollId> {
    match routing {
        ScrollRouting::Handled { node, .. } => Some(node),
        ScrollRouting::Unhandled => None,
    }
}

/// Returns the vertical offset of the last scroll event of `node` in `events`.
pub(super) fn offset_of(events: &[CompositorEvent], node: ScrollId) -> f32 {
    Stage::last_scroll(events, node).expect("the node moved").0[1]
}
