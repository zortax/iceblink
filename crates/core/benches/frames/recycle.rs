//! Scrolling a virtual list one row at a time, and the cost of each scroll's commit and frame.
//!
//! Each scroll replaces the row that scrolls out with a new row at the end, or moves it there and
//! records its labels again. A shifted list also moves the chunks of every other row with each
//! scroll.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use iceblink::{
    BufferAge, Built, ChunkKey, ClientId, ClipId, ClipNode, ClipShape, Color, FontId, Frame,
    FrameClock, FrameOutcome, FrameRequest, FrameStats, OrderEntry, OrderId, OrderNode, OutputId,
    Paint, PaintId, PaintKind, ROUND, ScrollId, ScrollNode, Transaction, TransformId,
};
use iceblink::{Compositor, OutputConfig};
use iceblink_core::CompositorImpl;
use iceblink_text::SwashRaster;

use crate::items::{node, props, quad, run};
use crate::layout::Layout;
use crate::list::{CAPS, ROW, SIDE};

/// The rows the list holds at a time, which cover the port and a few rows past it.
const LIVE: u64 = 44;

/// The number of cells of a row, each a label with an icon.
const CELLS: u64 = 3;

/// The fate of the row a scroll moves out.
#[derive(Clone, Copy)]
pub(crate) enum Departing {
    /// Removal of the row, and a new row with new keys and order nodes at the end.
    Replaced,
    /// A move of the row's order node to the end and of its chunks by their offsets, with its
    /// labels recorded again under the same keys.
    Reused,
    /// A replacement as for `Replaced`, with the content scrolled by a fraction of a row and the
    /// chunks of every other row moved by their offsets, so the rows keep their places in the
    /// port.
    Shifted,
}

/// How far the content of a shifted list scrolls per row it moves past.
// Whole pixels, as a client that snaps its layout to pixels moves it.
const SHIFT_STEP: f32 = 5.0;

/// One live row of the list.
struct Row {
    /// The position the row's chunks were first recorded at, which also owns their keys.
    first: u64,
    /// The order nodes the row made, the row's own last.
    nodes: Vec<OrderId>,
    /// How far the content was moved past the row's place when its chunks were recorded.
    base: f32,
}

/// A compositor drawing a virtual list, whose rows are each a background, and labels with icons
/// in order nodes of their own.
pub(crate) struct Recycler {
    departing: Departing,
    compositor: CompositorImpl,
    client: ClientId,
    output: OutputId,
    frame: Frame,
    layout: Layout,
    font: FontId,
    scroll: ScrollId,
    content: TransformId,
    port: ClipId,
    list: OrderId,
    zebra: PaintId,
    black: PaintId,
    /// The live rows from the top.
    rows: VecDeque<Row>,
    /// The index of the next row a scroll adds.
    next: u64,
    /// How far a shifted list moved its content past the rows' places.
    shift: f32,
}

impl Recycler {
    /// Returns the list past its first frame.
    pub(crate) fn new(departing: Departing) -> Self {
        let mut compositor = CompositorImpl::new();
        compositor.use_glyph_raster(Arc::new(SwashRaster::new()));
        let font = compositor
            .font_registry()
            .register(iceblink_testing::fonts::read_font(
                iceblink_testing::fonts::LATIN,
            ));
        let (client, _) = compositor.register_client();
        let output = compositor.create_output(OutputConfig {
            size: SIDE,
            scale: 1.0,
            opaque: true,
        });

        let mut tx = compositor.transaction(client);
        let space = tx.create_transform(node(None, None));
        let root = tx.create_order(OrderNode::default());
        tx.set_root(output, Some(root));
        let [width, height] = SIDE.map(|side| side as f32);
        let scroll = tx.create_scroll(ScrollNode {
            space,
            port: [0.0, 0.0, width, height],
            content: [width, 1.0e7],
            offset: [0.0, 0.0],
            behaviour: None,
        });
        let content = tx.create_transform(node(Some(space), Some(scroll)));
        let port = tx.create_clip(ClipNode {
            parent: None,
            space,
            shape: ClipShape::RoundedRect {
                rect: [0.0, 0.0, width, height],
                radii: [0.0; 8],
                shapes: ROUND,
            },
        });
        let white = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));
        let zebra = tx.create_paint(Paint::Solid(Color([0.92, 0.92, 0.96, 1.0])));
        let black = tx.create_paint(Paint::Solid(Color([0.0, 0.0, 0.0, 1.0])));

        let background = ChunkKey::new(u64::MAX, 0);
        let mut chunk = tx.record(background, props(space, None));
        let fill = chunk.paint_ref(PaintKind::SOLID, white);
        chunk.quad(quad([0.0, 0.0, width, height], fill));
        let list = tx.create_order(OrderNode::default());
        tx.splice(
            root,
            0,
            0,
            &[OrderEntry::Chunk(background), OrderEntry::Node(list)],
        );

        let mut recycler = Self {
            departing,
            compositor,
            client,
            output,
            frame: Frame::default(),
            layout: Layout::new(),
            font,
            scroll,
            content,
            port,
            list,
            zebra,
            black,
            rows: VecDeque::new(),
            next: 0,
            shift: 0.0,
        };

        for _ in 0..LIVE {
            recycler.add_row(&mut tx);
        }
        recycler.compositor.commit(tx).expect("the list commits");
        assert_eq!(recycler.draw(), Built::Frame);
        recycler
    }

    /// Commits a scroll by one row, which moves the top row's content to the end.
    // Out of line, so instruction profiles can count each call.
    #[inline(never)]
    pub(crate) fn commit(&mut self) {
        let mut tx = self.compositor.transaction(self.client);
        let top = self.rows.pop_front().expect("the list has rows");
        tx.splice(self.list, 0, 1, &[]);
        match self.departing {
            Departing::Replaced => {
                self.remove_row(&mut tx, &top);
                self.add_row(&mut tx);
            }
            Departing::Reused => self.reuse(&mut tx, top),
            Departing::Shifted => {
                self.remove_row(&mut tx, &top);
                self.shift += SHIFT_STEP - ROW;
                self.shift_rows(&mut tx);
                self.add_row(&mut tx);
            }
        }

        let first = (self.next - LIVE) as f32 * ROW;
        tx.scroll_to(self.scroll, [0.0, first + self.shift]);
        self.compositor.commit(tx).expect("the scroll commits");
    }

    /// Removes the chunks and order nodes of `row`.
    fn remove_row(&self, tx: &mut Transaction, row: &Row) {
        for part in 0..=2 * CELLS {
            tx.remove_chunk(ChunkKey::new(row.first, part as u32));
        }
        for &node in &row.nodes {
            tx.remove(node);
        }
    }

    /// Moves the chunks of every live row by the shift since they were recorded.
    fn shift_rows(&self, tx: &mut Transaction) {
        for row in &self.rows {
            let offset = [0.0, self.shift - row.base];
            for part in 0..=2 * CELLS {
                tx.set_offset(ChunkKey::new(row.first, part as u32), offset);
            }
        }
    }

    /// Moves `row` to the end of the list as the next row.
    fn reuse(&mut self, tx: &mut Transaction, row: Row) {
        let index = self.next;
        self.next += 1;

        let offset = [0.0, (index - row.first) as f32 * ROW];
        tx.set_offset(ChunkKey::new(row.first, 0), offset);
        for cell in 0..CELLS {
            tx.set_offset(ChunkKey::new(row.first, (1 + 2 * cell) as u32), offset);
            self.record_label(tx, row.first, index, cell);
        }

        let node = *row.nodes.last().expect("the row has its own node");
        tx.splice(
            self.list,
            self.rows.len() as u32,
            0,
            &[OrderEntry::Node(node)],
        );
        self.rows.push_back(row);
    }

    /// Records the label of cell `cell` of the row at `index`, under the keys of the row first
    /// recorded at `first`, and returns the label's key.
    fn record_label(&self, tx: &mut Transaction, first: u64, index: u64, cell: u64) -> ChunkKey {
        let x = 6.0 + cell as f32 * 260.0;
        let y = index as f32 * ROW + self.shift;
        let label = ChunkKey::new(first, (2 + 2 * cell) as u32);
        let mut chunk = tx.record(label, props(self.content, Some(self.port)));
        let fill = chunk.paint_ref(PaintKind::SOLID, self.black);
        let text = format!("Row {index}, cell {cell}");
        let glyphs = chunk.glyphs(&self.layout.line(&text, 12.0, [x + 14.0, y + 11.0]));
        chunk.glyph_run(run(self.font, glyphs, fill));
        label
    }

    /// Records the next row and appends it to the list.
    fn add_row(&mut self, tx: &mut Transaction) {
        let index = self.next;
        self.next += 1;
        let y = index as f32 * ROW + self.shift;
        let row_props = props(self.content, Some(self.port));

        let background = ChunkKey::new(index, 0);
        let mut chunk = tx.record(background, row_props);
        let fill = chunk.paint_ref(PaintKind::SOLID, self.zebra);
        chunk.quad(quad([0.0, y, SIDE[0] as f32, ROW], fill));
        let mut entries = vec![OrderEntry::Chunk(background)];
        let mut nodes = Vec::new();
        for cell in 0..CELLS {
            let x = 6.0 + cell as f32 * 260.0;
            let icon = ChunkKey::new(index, (1 + 2 * cell) as u32);
            let mut chunk = tx.record(icon, row_props);
            let fill = chunk.paint_ref(PaintKind::SOLID, self.black);
            chunk.quad(quad([x, y + 3.0, 9.0, 9.0], fill));
            let decoration = tx.create_order(OrderNode::default());
            tx.splice(decoration, 0, 0, &[OrderEntry::Chunk(icon)]);

            let label = self.record_label(tx, index, index, cell);
            let cell = tx.create_order(OrderNode::default());
            tx.splice(
                cell,
                0,
                0,
                &[OrderEntry::Chunk(label), OrderEntry::Node(decoration)],
            );
            entries.push(OrderEntry::Node(cell));
            nodes.extend([decoration, cell]);
        }

        let row = tx.create_order(OrderNode::default());
        tx.splice(row, 0, 0, &entries);
        nodes.push(row);
        tx.splice(
            self.list,
            self.rows.len() as u32,
            0,
            &[OrderEntry::Node(row)],
        );
        self.rows.push_back(Row {
            first: index,
            nodes,
            base: self.shift,
        });
    }

    /// Builds a frame and reports it presented.
    // Out of line, so instruction profiles can count each call.
    #[inline(never)]
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

    /// Returns the time `iterations` scroll commits take, with their frames built outside it.
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

    /// Returns the time the frames of `iterations` scrolls take, with their commits outside it.
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

    /// Prints the work counters of one scroll's commit and of its frame.
    pub(crate) fn report(&mut self, name: &str) {
        // The first scrolls draw rows no frame drew before.
        for _ in 0..LIVE {
            self.commit();
            self.draw();
        }
        let before = self.work();
        self.commit();
        let commit = self.work();
        assert_eq!(self.draw(), Built::Frame);
        let frame = self.work();

        let delta = |from: &[(&'static str, u64)], to: &[(&'static str, u64)]| {
            to.iter()
                .zip(from)
                .map(|((name, after), (_, before))| format!("{name} {}", after - before))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let counters = self.compositor.counters().frame;
        println!("{name}: commit: {}", delta(&before, &commit));
        println!("{name}: frame: {}", delta(&commit, &frame));
        println!(
            "{name}: frame: chunks_drawn {}, items_drawn {}, items_culled {}, \
             order_nodes_visited {}, damage_px {}, shifted_px {}, shift_refusals {}",
            counters.chunks_drawn,
            counters.items_drawn,
            counters.items_culled,
            counters.order_nodes_visited,
            counters.damage_px,
            counters.shifted_px,
            counters.shift_refusals,
        );
    }

    /// Returns the compositor's work counters.
    fn work(&self) -> Vec<(&'static str, u64)> {
        let mut work = Vec::new();
        self.compositor.extra_stats(&mut |name, value| {
            if name.starts_with("work.") {
                work.push((name, value));
            }
        });
        work
    }
}
