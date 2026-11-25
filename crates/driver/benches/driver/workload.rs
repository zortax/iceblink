//! The workload every mode runs: a drag over a long list whose client recycles rows.

use std::sync::Arc;

use iceblink::{
    Anchor, BorderStyle, ChunkFlags, ChunkKey, ChunkProps, ClientId, ClipRef, Color,
    CompositorEvent, FrameClock, GesturePhase, Matrix4, OrderEntry, OrderNode, OutputId, Paint,
    PaintId, PaintKind, PaintRef, Quad, ROUND, ScrollBehaviour, ScrollGesture, ScrollId,
    ScrollNode, ScrollPhysics, ScrollRouting, ScrollSource, SnapKind, Sources, SpaceRef,
    Transaction, TransformId, TransformNode,
};
use iceblink::{ClientEvent, CompositorClient, OutputConfig, RendererFactory, ScrollWatch};
use iceblink_testing::fakes::NullRenderer;

/// The output's size in pixels.
const SIDE: [u32; 2] = [800, 600];

/// The rows of the list, how many one chunk holds, and the height of a row.
const ROWS: u32 = 2_000;
const PER_CHUNK: u32 = 25;
const ROW: f32 = 24.0;

/// How many frames apart the client recycles a chunk of rows.
const RECYCLE: u64 = 4;

/// A client of one output that shows a long list.
pub(crate) struct Workload {
    client: ClientId,
    output: OutputId,
    node: ScrollId,
    content: TransformId,
    paints: [PaintId; 2],
    events: Vec<ClientEvent>,
    /// What the client saw: frames, commits answered and the newest offset.
    pub(crate) seen: Seen,
    /// The watch and frame events the client took.
    pub(crate) notices: u64,
}

/// What a run of the workload produced, which every mode must agree on.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Seen {
    pub(crate) frames: u64,
    pub(crate) committed: u64,
    pub(crate) offset: f32,
}

impl Workload {
    /// Records the list for a new client of `driver` on an output drawn by a null renderer.
    pub(crate) fn open(driver: &mut dyn CompositorClient) -> Self {
        let (client, _) = driver.register_client();
        let renderer: RendererFactory = Box::new(|| Ok(Box::new(NullRenderer::new())));
        let output = driver.create_output(
            OutputConfig {
                size: SIDE,
                scale: 1.0,
                opaque: true,
            },
            renderer,
        );
        let mut tx = driver.transaction(client);
        let space = tx.create_transform(node(None, None));
        let root = tx.create_order(OrderNode::default());
        tx.set_root(output, Some(root));
        let scroll = tx.create_scroll(ScrollNode {
            space,
            port: [0.0, 0.0, SIDE[0] as f32, SIDE[1] as f32],
            content: [SIDE[0] as f32, ROWS as f32 * ROW],
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
        let content = tx.create_transform(node(Some(space), Some(scroll)));
        let paints = [
            tx.create_paint(Paint::Solid(Color([0.9, 0.93, 0.99, 1.0]))),
            tx.create_paint(Paint::Solid(Color([0.98, 0.98, 1.0, 1.0]))),
        ];
        let workload = Self {
            client,
            output,
            node: scroll,
            content,
            paints,
            events: Vec::new(),
            seen: Seen::default(),
            notices: 0,
        };
        let chunks = ROWS / PER_CHUNK;
        for chunk in 0..chunks {
            workload.record(&mut tx, chunk, 0);
        }
        let entries: Vec<_> = (0..chunks)
            .map(|chunk| OrderEntry::Chunk(key(chunk)))
            .collect();
        tx.splice(root, 0, 0, &entries);
        driver.commit(tx);
        workload
    }

    pub(crate) fn client(&self) -> ClientId {
        self.client
    }

    /// Watches every frame drawn and every row the drag moves past.
    pub(crate) fn watch(&self, driver: &mut dyn CompositorClient) {
        let watch = ScrollWatch {
            stride: [0.0, 6.0],
            ..ScrollWatch::default()
        };
        driver.watch_scroll(self.client, self.node, Some(watch));
        driver.watch_frames(self.client, self.output, true);
    }

    /// Runs frame `index` of the workload at `clock`: the client's work, then the step.
    ///
    /// Every frame drags the list by a touchpad step, and every few frames recycle a chunk of
    /// rows.
    pub(crate) fn frame(
        &mut self,
        driver: &mut dyn CompositorClient,
        index: u64,
        clock: FrameClock,
    ) {
        let gesture = ScrollGesture {
            position: [400.0, 300.0],
            delta: [0.0, 6.0],
            phase: drag(index),
            source: ScrollSource::Touchpad,
            time_ns: clock.now_ns,
        };
        let predicted = ScrollRouting::Handled {
            node: self.node,
            client: self.client,
        };
        driver.scroll_input(self.client, self.output, gesture, predicted);
        if index.is_multiple_of(RECYCLE) {
            let mut tx = driver.transaction(self.client);
            let chunk = (index / RECYCLE) as u32 % (ROWS / PER_CHUNK);
            self.record(&mut tx, chunk, index);
            driver.commit(tx);
        }
        driver.step(clock);
    }

    /// Takes the client's events into what it saw.
    pub(crate) fn take(&mut self, driver: &mut dyn CompositorClient) {
        driver.drain_events(self.client, &mut self.events);
        for event in self.events.drain(..) {
            match event {
                ClientEvent::Committed { .. } => self.seen.committed += 1,
                ClientEvent::Compositor(CompositorEvent::Scroll { offset, .. }) => {
                    self.seen.offset = offset[1];
                }
                ClientEvent::Watched { .. } | ClientEvent::Drawn { .. } => self.notices += 1,
                other => panic!("the workload queued {other:?}"),
            }
        }
        self.seen.frames = driver.stats().frames_built;
    }

    /// Records chunk `chunk` of rows, coloured by `generation`.
    fn record(&self, tx: &mut Transaction, chunk: u32, generation: u64) {
        let props = ChunkProps {
            space: self.content,
            clip: None,
            flags: ChunkFlags::default(),
        };
        let mut recorder = tx.record(key(chunk), props);
        let fills = [
            recorder.paint_ref(PaintKind::SOLID, self.paints[0]),
            recorder.paint_ref(PaintKind::SOLID, self.paints[1]),
        ];
        for row in chunk * PER_CHUNK..(chunk + 1) * PER_CHUNK {
            let parity = (u64::from(row) + generation) as usize % 2;
            recorder.quad(quad(
                [
                    8.0,
                    row as f32 * ROW + 2.0,
                    SIDE[0] as f32 - 16.0,
                    ROW - 4.0,
                ],
                fills[parity],
            ));
        }
    }
}

/// Returns the phase of the drag's gesture at frame `index`.
fn drag(index: u64) -> GesturePhase {
    if index == 0 {
        GesturePhase::Begin
    } else {
        GesturePhase::Update
    }
}

fn key(chunk: u32) -> ChunkKey {
    ChunkKey::new(100 + u64::from(chunk), 0)
}

fn node(parent: Option<TransformId>, scroll: Option<ScrollId>) -> TransformNode {
    TransformNode {
        parent,
        local: Matrix4::IDENTITY,
        scroll,
        anchor: Anchor::Scrolls,
    }
}

fn quad(bounds: [f32; 4], fill: PaintRef) -> Quad {
    Quad {
        bounds,
        radii: [4.0; 8],
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
