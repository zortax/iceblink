//! A compositor drawing a scrolled list of text rows over an opaque background.

use std::sync::Arc;

use iceblink::{
    BufferAge, Built, ChunkKey, ClientId, ClipNode, ClipShape, Color, Frame, FrameClock,
    FrameOutcome, FrameRequest, FrameStats, OrderEntry, OrderNode, OutputId, Paint, PaintId,
    PaintKind, ROUND, RenderCapabilities, ScrollId, ScrollNode, TransformId,
};
use iceblink::{Compositor, OutputConfig};
use iceblink_core::CompositorImpl;
use iceblink_text::SwashRaster;

use crate::items::{node, props, quad, run};
use crate::layout::Layout;

/// The capabilities of a renderer with every fast path.
pub(crate) const CAPS: RenderCapabilities = RenderCapabilities {
    max_texture_size: 8192,
    subpixel_text: false,
    custom_shaders: true,
    vector_compute: false,
    can_shift: true,
    resident_chunks: true,
    blend_modes: 1,
    kept_rasters: false,
};

/// The output's size in pixels.
pub(crate) const SIDE: [u32; 2] = [800, 600];

/// The height of one row of the list.
pub(crate) const ROW: f32 = 15.0;

/// The rows a list holds.
#[derive(Clone, Copy)]
pub(crate) struct Rows {
    pub(crate) count: u64,
    /// Every how many rows one has a background quad.
    pub(crate) quad_every: u64,
}

/// A compositor drawing a list of text rows in a scrolled content space under a port clip,
/// over an opaque background.
pub(crate) struct List {
    pub(crate) compositor: CompositorImpl,
    pub(crate) client: ClientId,
    pub(crate) output: OutputId,
    pub(crate) frame: Frame,
    /// The transform node the port is measured in.
    pub(crate) space: TransformId,
    pub(crate) scroll: ScrollId,
    pub(crate) rows: Rows,
    /// The paint only row 3 uses.
    pub(crate) marked: PaintId,
}

impl List {
    /// Returns the list past its first frame.
    pub(crate) fn new(rows: Rows) -> Self {
        let mut list = Self::committed(rows);
        assert_eq!(list.draw(), Built::Frame);
        list
    }

    /// Returns the list committed, with no frame built.
    pub(crate) fn committed(rows: Rows) -> Self {
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
        let layout = Layout::new();

        let mut tx = compositor.transaction(client);
        let space = tx.create_transform(node(None, None));
        let root = tx.create_order(OrderNode::default());
        tx.set_root(output, Some(root));
        let [width, height] = SIDE.map(|side| side as f32);
        let scroll = tx.create_scroll(ScrollNode {
            space,
            port: [0.0, 0.0, width, height],
            content: [width, rows.count as f32 * ROW],
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
        let marked = tx.create_paint(Paint::Solid(Color([0.1, 0.2, 0.8, 1.0])));

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

        let mut entries = Vec::new();
        for row in 0..rows.count {
            let key = ChunkKey::new(row, 0);
            let mut chunk = tx.record(key, props(content, Some(port)));
            let y = row as f32 * ROW;
            if row % rows.quad_every == 0 {
                let fill = chunk.paint_ref(PaintKind::SOLID, zebra);
                chunk.quad(quad([0.0, y, width, ROW], fill));
            }
            let ink = if row == 3 { marked } else { black };
            let fill = chunk.paint_ref(PaintKind::SOLID, ink);
            let text = format!("Row {row}: the quick brown fox jumps over the lazy dog");
            let glyphs = chunk.glyphs(&layout.line(&text, 12.0, [6.0, y + 11.0]));
            chunk.glyph_run(run(font, glyphs, fill));
            entries.push(OrderEntry::Chunk(key));
        }
        tx.splice(list, 0, 0, &entries);
        compositor.commit(tx).expect("the list commits");

        Self {
            compositor,
            client,
            output,
            frame: Frame::default(),
            space,
            scroll,
            rows,
            marked,
        }
    }

    /// Returns the largest scroll offset of the list.
    pub(crate) fn end(&self) -> f32 {
        self.rows.count as f32 * ROW - SIDE[1] as f32
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
}
