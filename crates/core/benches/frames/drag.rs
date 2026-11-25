//! Dragging the edge between two panes, which writes the clip of each pane on every step.

use std::time::{Duration, Instant};

use iceblink::{
    BufferAge, Built, ChunkKey, ClientId, ClipId, ClipNode, ClipShape, Color, Frame, FrameClock,
    FrameOutcome, FrameRequest, FrameStats, OrderEntry, OrderNode, OutputId, Paint, PaintKind,
    ROUND, TransformId,
};
use iceblink::{Compositor, OutputConfig};
use iceblink_core::CompositorImpl;

use crate::items::{node, props, quad};
use crate::list::CAPS;

/// The output's size in pixels.
const SIDE: [f32; 2] = [1600.0, 1000.0];

/// The height of the toolbar above both panes.
const TOP: f32 = 42.0;

/// The number of columns of the upper pane.
const COLUMNS: u64 = 4;

/// The number of rows of each column.
const ROWS: u32 = 50;

/// The height of one row of the upper pane.
const ROW: f32 = 20.0;

/// The number of chunks of the dock, the lower pane.
const DOCK: u32 = 10;

/// How far each step moves the edge.
const STEP: f32 = 5.0;

/// The range the edge moves in.
const EDGE: [f32; 2] = [300.0, 900.0];

/// Two panes under a toolbar: an upper pane of 200 row chunks in four columns and a dock below
/// it, each drawn through its own clip.
///
/// Every step moves the edge between the panes by 5 pixels, and the rows reach past the edge, so
/// the edge cuts them.
pub(crate) struct Drag {
    compositor: CompositorImpl,
    client: ClientId,
    output: OutputId,
    frame: Frame,
    space: TransformId,
    upper: ClipId,
    dock: ClipId,
    edge: f32,
    step: f32,
}

impl Drag {
    /// Returns the panes past their first frame, with the edge at 687.
    pub(crate) fn new() -> Self {
        let mut compositor = CompositorImpl::new();
        let (client, _) = compositor.register_client();
        let output = compositor.create_output(OutputConfig {
            size: SIDE.map(|side| side as u32),
            scale: 1.0,
            opaque: true,
        });

        let mut tx = compositor.transaction(client);
        let space = tx.create_transform(node(None, None));
        let root = tx.create_order(OrderNode::default());
        tx.set_root(output, Some(root));
        let edge = TOP + 645.0;
        let upper = tx.create_clip(pane(space, TOP, edge));
        let dock = tx.create_clip(pane(space, edge, SIDE[1]));
        let white = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));
        let zebra = tx.create_paint(Paint::Solid(Color([0.92, 0.92, 0.96, 1.0])));
        let label = tx.create_paint(Paint::Solid(Color([0.2, 0.2, 0.3, 1.0])));

        let mut entries = Vec::new();
        let background = ChunkKey::new(u64::MAX, 0);
        let mut chunk = tx.record(background, props(space, None));
        let fill = chunk.paint_ref(PaintKind::SOLID, white);
        chunk.quad(quad([0.0, 0.0, SIDE[0], SIDE[1]], fill));
        entries.push(OrderEntry::Chunk(background));

        let width = SIDE[0] / COLUMNS as f32;
        for column in 0..COLUMNS {
            for row in 0..ROWS {
                let key = ChunkKey::new(column, row);
                let mut chunk = tx.record(key, props(space, Some(upper)));
                let [x, y] = [column as f32 * width, TOP + row as f32 * ROW];
                let paint = if row % 2 == 0 { zebra } else { white };
                let fill = chunk.paint_ref(PaintKind::SOLID, paint);
                chunk.quad(quad([x, y, width, ROW], fill));
                let fill = chunk.paint_ref(PaintKind::SOLID, label);
                chunk.quad(quad([x + 8.0, y + 4.0, width * 0.6, ROW - 8.0], fill));
                entries.push(OrderEntry::Chunk(key));
            }
        }
        for part in 0..DOCK {
            let key = ChunkKey::new(COLUMNS, part);
            let mut chunk = tx.record(key, props(space, Some(dock)));
            let x = part as f32 * SIDE[0] / DOCK as f32;
            let fill = chunk.paint_ref(PaintKind::SOLID, zebra);
            chunk.quad(quad([x, TOP, SIDE[0] / DOCK as f32, SIDE[1]], fill));
            entries.push(OrderEntry::Chunk(key));
        }
        tx.splice(root, 0, 0, &entries);
        compositor.commit(tx).expect("the panes commit");

        let mut drag = Self {
            compositor,
            client,
            output,
            frame: Frame::default(),
            space,
            upper,
            dock,
            edge,
            step: -STEP,
        };
        assert_eq!(drag.draw(), Built::Frame);
        drag
    }

    /// Commits a move of the edge by one step.
    pub(crate) fn commit(&mut self) {
        if !(EDGE[0]..=EDGE[1]).contains(&(self.edge + self.step)) {
            self.step = -self.step;
        }
        self.edge += self.step;

        let mut tx = self.compositor.transaction(self.client);
        tx.set_clip(self.upper, pane(self.space, TOP, self.edge));
        tx.set_clip(self.dock, pane(self.space, self.edge, SIDE[1]));
        self.compositor.commit(tx).expect("the drag commits");
    }

    /// Builds a frame and reports it presented.
    pub(crate) fn draw(&mut self) -> Built {
        let request = FrameRequest {
            caps: &CAPS,
            age: BufferAge::Kept,
            clock: FrameClock::default(),
            subpixel: None,
        };
        let built = self
            .compositor
            .build_frame(self.output, &request, &mut self.frame);
        if built == Built::Frame {
            let outcome = FrameOutcome::Presented(FrameStats::default());
            self.compositor
                .frame_done(self.output, self.frame.id, &outcome);
        }
        built
    }

    /// Returns the time `iterations` drag commits take, with their frames built outside it.
    pub(crate) fn time_commits(&mut self, iterations: u64) -> Duration {
        let mut total = Duration::ZERO;
        for _ in 0..iterations {
            let start = Instant::now();
            self.commit();
            total += start.elapsed();
            self.draw();
        }
        total
    }

    /// Returns the time the frames of `iterations` drag steps take, with their commits outside
    /// it.
    pub(crate) fn time_frames(&mut self, iterations: u64) -> Duration {
        let mut total = Duration::ZERO;
        for _ in 0..iterations {
            self.commit();
            let start = Instant::now();
            std::hint::black_box(self.draw());
            total += start.elapsed();
        }
        total
    }

    /// Prints the mean damage and drawn items of the frames of 120 drag steps.
    pub(crate) fn report(&mut self, name: &str) {
        const STEPS: u64 = 120;
        let (mut damage, mut items) = (0, 0);
        for _ in 0..STEPS {
            self.commit();
            assert_eq!(self.draw(), Built::Frame);
            let counters = self.compositor.counters().frame;
            damage += counters.damage_px;
            items += u64::from(counters.items_drawn);
        }
        println!(
            "{name}: frame: mean damage_px {}, mean items_drawn {}",
            damage / STEPS,
            items / STEPS,
        );
    }
}

/// Returns a clip over the full width of the output, from `top` to `bottom`.
fn pane(space: TransformId, top: f32, bottom: f32) -> ClipNode {
    ClipNode {
        parent: None,
        space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, top, SIDE[0], bottom - top],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    }
}
