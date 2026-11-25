//! A table of orders in a scrolled card, with an order node for every box.
//!
//! A search hides the rows and shows them again all at once, and a status filter a quarter at a
//! time. Only the rows in the port record their chunks. The other rows list keys that nothing
//! recorded.
//!
//! Each change records the page around the table again under a new clip. It removes the rows it
//! hides with every node of theirs, emptying each node first.

mod change;
mod row;
mod timing;

use std::sync::Arc;

use iceblink::{
    Built, ChunkKey, ClientId, ClipId, ClipNode, ClipShape, Color, FontId, Frame, OrderEntry,
    OrderId, OrderNode, OutputId, Paint, PaintId, PaintKind, ROUND, ScrollId, ScrollNode,
    Transaction, TransformId,
};
use iceblink::{Compositor, OutputConfig};
use iceblink_core::CompositorImpl;
use iceblink_text::SwashRaster;

use self::row::Row;
use crate::items::{node, props, quad, run};
use crate::layout::Layout;

/// The output's size in pixels.
const SIDE: [u32; 2] = [1280, 800];

/// The height of one row.
const ROW: f32 = 48.0;

/// The number of chunks of the page around the table.
const PAGE: u64 = 100;

/// The number of entries of the table's node before its rows.
const HEAD: u32 = 2;

/// Which rows a change hides and shows again.
#[derive(Clone, Copy)]
pub(crate) enum Change {
    /// Every row, as a search that matches no order.
    Search,
    /// Every fourth row, as a status filter.
    Filter,
}

/// A compositor drawing the table, with the rows of its change shown or hidden.
pub(crate) struct Table {
    change: Change,
    /// The number of rows of the full table.
    count: usize,
    compositor: CompositorImpl,
    client: ClientId,
    output: OutputId,
    frame: Frame,
    layout: Layout,
    font: FontId,
    space: TransformId,
    content: TransformId,
    scroll: ScrollId,
    port: ClipId,
    /// The clip the page is recorded under.
    page_clip: ClipId,
    table: OrderId,
    zebra: PaintId,
    black: PaintId,
    /// How many rows from the top lie in the port.
    visible: usize,
    /// The shown rows, from the top.
    rows: Vec<Row>,
    /// Whether the change's rows are hidden.
    hidden: bool,
    /// The owner of the next row's chunk keys.
    next: u64,
}

impl Table {
    /// Returns the table of `count` rows with every row shown, past its first frame.
    pub(crate) fn new(change: Change, count: usize) -> Self {
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
            port: [240.0, 64.0, width - 240.0, height - 64.0],
            content: [width - 240.0, count as f32 * ROW],
            offset: [0.0, 0.0],
            behaviour: None,
        });
        let content = tx.create_transform(node(Some(space), Some(scroll)));
        let port = tx.create_clip(ClipNode {
            parent: None,
            space,
            shape: ClipShape::RoundedRect {
                rect: [240.0, 64.0, width - 240.0, height - 64.0],
                radii: [0.0; 8],
                shapes: ROUND,
            },
        });
        let page_clip = tx.create_clip(page_clip(space));
        let white = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));
        let zebra = tx.create_paint(Paint::Solid(Color([0.92, 0.92, 0.96, 1.0])));
        let black = tx.create_paint(Paint::Solid(Color([0.0, 0.0, 0.0, 1.0])));

        let background = ChunkKey::new(u64::MAX, 0);
        let mut chunk = tx.record(background, props(space, None));
        let fill = chunk.paint_ref(PaintKind::SOLID, white);
        chunk.quad(quad([0.0, 0.0, width, height], fill));
        let page = tx.create_order(OrderNode::default());
        let table = tx.create_order(OrderNode::default());
        tx.splice(
            root,
            0,
            0,
            &[OrderEntry::Chunk(background), OrderEntry::Node(page)],
        );
        let mut entries: Vec<OrderEntry> = (0..PAGE)
            .map(|index| OrderEntry::Chunk(page_key(index)))
            .collect();
        entries.push(OrderEntry::Node(table));
        tx.splice(page, 0, 0, &entries);
        let head: Vec<OrderEntry> = (0..u64::from(HEAD))
            .map(|index| OrderEntry::Chunk(ChunkKey::new(u64::MAX - 1, index as u32)))
            .collect();
        tx.splice(table, 0, 0, &head);

        let mut built = Self {
            change,
            count,
            compositor,
            client,
            output,
            frame: Frame::default(),
            layout: Layout::new(),
            font,
            space,
            content,
            scroll,
            port,
            page_clip,
            table,
            zebra,
            black,
            visible: ((height - 64.0) / ROW).ceil() as usize,
            rows: Vec::new(),
            hidden: false,
            next: 0,
        };

        built.record_page(&mut tx);
        let rows: Vec<Row> = (0..count).map(|_| built.make_row(&mut tx)).collect();
        let nodes: Vec<OrderEntry> = rows
            .iter()
            .map(|row| OrderEntry::Node(row.node()))
            .collect();
        tx.splice(table, HEAD, 0, &nodes);
        built.rows = rows;
        built.draw_rows(&mut tx);
        built.compositor.commit(tx).expect("the table commits");
        assert_eq!(built.draw(), Built::Frame);
        built
    }

    /// Commits the next change: hides the change's rows when they are shown, and shows new
    /// rows in their place when they are hidden.
    // Out of line, so instruction profiles can count each call.
    #[inline(never)]
    pub(crate) fn commit(&mut self) {
        let mut tx = self.compositor.transaction(self.client);
        let old_clip = self.page_clip;
        self.page_clip = tx.create_clip(page_clip(self.space));
        self.record_page(&mut tx);
        tx.remove(old_clip);

        if self.hidden {
            self.show(&mut tx);
        } else {
            self.hide(&mut tx);
        }
        self.hidden = !self.hidden;

        tx.set_scroll_node(
            self.scroll,
            ScrollNode {
                space: self.space,
                port: [240.0, 64.0, SIDE[0] as f32 - 240.0, SIDE[1] as f32 - 64.0],
                content: [SIDE[0] as f32 - 240.0, self.rows.len() as f32 * ROW],
                offset: [0.0, 0.0],
                behaviour: None,
            },
        );
        self.draw_rows(&mut tx);
        self.compositor.commit(tx).expect("the change commits");
    }

    /// Commits the search field recorded again with its caret shown or hidden, as a caret
    /// blinks.
    // Out of line, so instruction profiles can count each call.
    #[inline(never)]
    pub(crate) fn blink(&mut self) {
        let mut tx = self.compositor.transaction(self.client);
        let mut chunk = tx.record(page_key(0), props(self.space, Some(self.page_clip)));
        let fill = chunk.paint_ref(PaintKind::SOLID, self.black);
        let text = self.layout.line("Search customers", 14.0, [8.0, 20.0]);
        let glyphs = chunk.glyphs(&text);
        chunk.glyph_run(run(self.font, glyphs, fill));
        if self.next.is_multiple_of(2) {
            chunk.quad(quad([130.0, 6.0, 1.0, 18.0], fill));
        }

        self.next += 1;
        self.compositor.commit(tx).expect("the field commits");
    }

    /// Records the page around the table under the page clip.
    fn record_page(&self, tx: &mut Transaction) {
        for index in 0..PAGE {
            let x = (index % 10) as f32 * 24.0;
            let y = (index / 10) as f32 * 30.0;
            let mut chunk = tx.record(page_key(index), props(self.space, Some(self.page_clip)));
            if index % 2 == 0 {
                let fill = chunk.paint_ref(PaintKind::SOLID, self.zebra);
                chunk.quad(quad([x, y, 22.0, 28.0], fill));
            } else {
                let fill = chunk.paint_ref(PaintKind::SOLID, self.black);
                let glyphs = chunk.glyphs(&self.layout.line("Revenue", 12.0, [x, y + 20.0]));
                chunk.glyph_run(run(self.font, glyphs, fill));
            }
        }
    }
}

/// Returns the key of the page's chunk `index`.
fn page_key(index: u64) -> ChunkKey {
    ChunkKey::new(u64::MAX - 2, index as u32)
}

/// Returns the clip the page is recorded under.
fn page_clip(space: TransformId) -> ClipNode {
    ClipNode {
        parent: None,
        space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 240.0, 800.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    }
}
