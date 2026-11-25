//! A threaded reference compositor showing a scrolling list.

use std::sync::Arc;

use iceblink::smallvec::smallvec;
use iceblink::{
    Anchor, Animation, AnimationId, AnimationStart, AnimationTarget, BlendMode, BorderStyle,
    ChunkFlags, ChunkKey, ChunkProps, ClientId, ClipRef, Color, CompositorEvent, Direction, Easing,
    EffectId, EffectNode, FillMode, GesturePhase, Keyframe, Keyframes, Matrix4, OrderEntry,
    OrderId, OrderNode, OutputId, Paint, PaintKind, PaintRef, Quad, ROUND, ScrollBehaviour,
    ScrollGesture, ScrollId, ScrollNode, ScrollPhase, ScrollPhysics, ScrollRouting, ScrollSource,
    SnapKind, Sources, SpaceRef, Timing, Transaction, TransformId, TransformNode,
};
use iceblink::{ClientEvent, CompositorClient, OutputConfig, RendererFactory};
use iceblink_core::CompositorImpl;
use iceblink_driver::now_ns;
use iceblink_driver::thread::{ClientHandle, ThreadedCompositor};

pub(crate) const MS: u64 = 1_000_000;

/// The refresh period of the fake display.
pub(crate) const PERIOD_NS: u64 = 10 * MS;

/// The height of a row of the list.
pub(crate) const ROW: f32 = 20.0;

/// The colours of the even and the odd rows.
pub(crate) const EVEN: [f32; 4] = [0.2, 0.4, 0.8, 1.0];
pub(crate) const ODD: [f32; 4] = [0.9, 0.9, 0.95, 1.0];

/// The chunk that holds the rows, one quad per row.
pub(crate) const ROWS: ChunkKey = ChunkKey::new(1, 0);

/// The chunk that holds the faded quad.
pub(crate) const FADED: ChunkKey = ChunkKey::new(2, 0);

/// Spawns the reference compositor on a driver thread.
pub(crate) fn spawn() -> (ThreadedCompositor, ClientHandle) {
    ThreadedCompositor::spawn(Box::new(CompositorImpl::new()))
}

/// A 200 by 200 output that is a port onto a list of 200 rows, which scrolls vertically with
/// kinetic physics.
pub(crate) struct List {
    pub(crate) client: ClientId,
    pub(crate) output: OutputId,
    pub(crate) root: OrderId,
    pub(crate) space: TransformId,
    pub(crate) node: ScrollId,
    entries: u32,
}

impl List {
    /// Opens the list on an output drawn by `renderer`, once its first commit is applied.
    pub(crate) fn open(handle: &mut ClientHandle, renderer: RendererFactory) -> Self {
        let (client, _) = handle.register_client();
        let output = handle.create_output(
            OutputConfig {
                size: [200, 200],
                scale: 1.0,
                opaque: true,
            },
            renderer,
        );
        let mut tx = handle.transaction(client);
        let space = tx.create_transform(TransformNode {
            parent: None,
            local: Matrix4::IDENTITY,
            scroll: None,
            anchor: Anchor::Scrolls,
        });
        let node = tx.create_scroll(ScrollNode {
            space,
            port: [0.0, 0.0, 200.0, 200.0],
            content: [200.0, 200.0 * ROW],
            offset: [0.0; 2],
            behaviour: Some(ScrollBehaviour {
                physics: ScrollPhysics::Kinetic,
                snap: Arc::from([]),
                snap_kind: SnapKind::Mandatory,
                bounce: Sources::NONE,
                user_scrollable: [false, true],
                chain: Sources::NONE,
            }),
        });
        let content = tx.create_transform(TransformNode {
            parent: Some(space),
            local: Matrix4::IDENTITY,
            scroll: Some(node),
            anchor: Anchor::Scrolls,
        });
        let root = tx.create_order(OrderNode::default());
        tx.set_root(output, Some(root));
        record_rows(&mut tx, content);
        let mut list = Self {
            client,
            output,
            root,
            space,
            node,
            entries: 0,
        };
        list.push(&mut tx, OrderEntry::Chunk(ROWS));
        commit(handle, tx);
        list
    }

    /// Appends an entry to the root's sequence in `tx`.
    pub(crate) fn push(&mut self, tx: &mut Transaction, entry: OrderEntry) {
        tx.splice(self.root, self.entries, 0, &[entry]);
        self.entries += 1;
    }

    /// Hands one step of a touchpad gesture in the middle of the list to the compositor, now,
    /// predicting that the list takes it.
    pub(crate) fn gesture(&self, handle: &mut ClientHandle, delta: f32, phase: GesturePhase) {
        let source = match phase {
            GesturePhase::Step => ScrollSource::Wheel,
            _ => ScrollSource::Touchpad,
        };
        let gesture = ScrollGesture {
            position: [100.0, 100.0],
            delta: [0.0, delta],
            phase,
            source,
            time_ns: now_ns(),
        };
        let predicted = ScrollRouting::Handled {
            node: self.node,
            client: self.client,
        };
        handle.scroll_input(self.client, self.output, gesture, predicted);
    }

    /// Drags the list 10 pixels down and releases it at `velocity` pixels per second.
    pub(crate) fn fling(&self, handle: &mut ClientHandle, velocity: f32) {
        self.gesture(handle, 10.0, GesturePhase::Begin);
        self.gesture(
            handle,
            0.0,
            GesturePhase::End {
                velocity: [0.0, velocity],
            },
        );
    }
}

/// Draws a quad over the list in a group whose opacity `iterations` animations of `duration_ns`
/// fade between 0.2 and 0.8, starting `delay_ns` after the next frame.
pub(crate) fn fade(
    handle: &mut ClientHandle,
    list: &mut List,
    duration_ns: u64,
    delay_ns: i64,
    iterations: f32,
) -> (EffectId, AnimationId) {
    let mut tx = handle.transaction(list.client);
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
    quads(
        &mut tx,
        FADED,
        props(list.space),
        &[[20.0, 20.0, 100.0, 100.0]],
    );
    tx.splice(group, 0, 0, &[OrderEntry::Chunk(FADED)]);
    list.push(&mut tx, OrderEntry::Node(group));
    let linear = |offset, value| Keyframe {
        offset,
        value,
        easing: Easing::Linear,
    };
    let animation = tx.animate(Animation {
        target: AnimationTarget::Opacity(effect),
        keyframes: Keyframes::Scalar(smallvec![linear(0.0, 0.2), linear(1.0, 0.8)]),
        timing: Timing {
            duration_ns,
            delay_ns,
            iterations,
            direction: Direction::Alternate,
            fill: FillMode::None,
            easing: Easing::Linear,
        },
        start: AnimationStart::Next,
    });
    commit(handle, tx);
    (effect, animation)
}

/// The serial, vertical offset and phase of every scroll event of `node` in `events`.
pub(crate) fn scrolls(events: &[ClientEvent], node: ScrollId) -> Vec<(u64, f32, ScrollPhase)> {
    events
        .iter()
        .filter_map(|event| match *event {
            ClientEvent::Compositor(CompositorEvent::Scroll {
                node: of,
                offset,
                serial,
                phase,
                ..
            }) if of == node => Some((serial, offset[1], phase)),
            _ => None,
        })
        .collect()
}

/// Commits `tx`, waits until the driver applied it, and returns the events of its client.
///
/// # Panics
///
/// Panics when the commit is rejected.
pub(crate) fn commit(handle: &mut ClientHandle, tx: Transaction) -> Vec<ClientEvent> {
    let client = tx.client();
    let id = handle.commit(tx);
    handle.sync();
    let mut events = Vec::new();
    handle.drain_events(client, &mut events);
    let answered = events.iter().any(|event| match event {
        ClientEvent::Committed { commit, .. } => *commit == id,
        ClientEvent::Rejected { commit, error } if *commit == id => {
            panic!("the commit is rejected: {error}")
        }
        _ => false,
    });
    assert!(answered, "the commit is answered");
    events
}

/// Records the rows in `space`, one quad per row, alternating between `EVEN` and `ODD`.
fn record_rows(tx: &mut Transaction, space: TransformId) {
    let even = tx.create_paint(Paint::Solid(Color(EVEN)));
    let odd = tx.create_paint(Paint::Solid(Color(ODD)));
    let mut chunk = tx.record(ROWS, props(space));
    let even = chunk.paint_ref(PaintKind::SOLID, even);
    let odd = chunk.paint_ref(PaintKind::SOLID, odd);
    for row in 0..200 {
        let fill = if row % 2 == 0 { even } else { odd };
        chunk.quad(quad([0.0, row as f32 * ROW, 200.0, ROW], fill));
    }
}

/// Chunk props in `space` with no clip.
pub(crate) fn props(space: TransformId) -> ChunkProps {
    ChunkProps {
        space,
        clip: None,
        flags: ChunkFlags::default(),
    }
}

/// Records chunk `key` in `tx` with one opaque quad per rectangle.
pub(crate) fn quads(tx: &mut Transaction, key: ChunkKey, props: ChunkProps, rects: &[[f32; 4]]) {
    let paint = tx.create_paint(Paint::Solid(Color([0.2, 0.4, 0.8, 1.0])));
    let mut chunk = tx.record(key, props);
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    for &rect in rects {
        chunk.quad(quad(rect, fill));
    }
}

/// A borderless quad over `bounds`.
pub(crate) fn quad(bounds: [f32; 4], fill: PaintRef) -> Quad {
    Quad {
        bounds,
        radii: [0.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill,
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        style: BorderStyle::SOLID,
        _pad: [0; 3],
    }
}
