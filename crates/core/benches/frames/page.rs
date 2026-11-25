//! A page of panels that scrolls its root element, with an order node for every box.

use std::sync::Arc;
use std::time::{Duration, Instant};

use iceblink::{
    Anchor, BlendMode, BufferAge, Built, ChunkKey, ClientId, ClipNode, ClipShape, Color,
    EffectNode, Frame, FrameClock, FrameOutcome, FrameRequest, FrameStats, OrderEntry, OrderNode,
    OutputId, Paint, PaintKind, ROUND, RenderCapabilities, ScrollId, ScrollNode,
};
use iceblink::{Compositor, OutputConfig};
use iceblink_core::CompositorImpl;
use iceblink_text::SwashRaster;

use crate::items::{node, props, quad, run};
use crate::layout::Layout;

/// The capabilities of a renderer with every fast path.
const CAPS: RenderCapabilities = RenderCapabilities {
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
const SIDE: [u32; 2] = [1920, 1080];

/// The width of the scrollbar gutter at the right edge of the output.
const GUTTER: f32 = 15.0;

/// The number of panels of the page, three to a row.
const PANELS: u64 = 100;

/// The number of boxes of a panel, three to a row.
const BOXES: u32 = 30;

/// Every how many boxes one is drawn at half opacity.
const FADED_EVERY: u64 = 40;

/// The size of a panel.
const PANEL: [f32; 2] = [620.0, 340.0];

/// The size of a box.
const BOX: [f32; 2] = [180.0, 24.0];

/// The distance between the origins of neighbouring boxes.
const PITCH: [f32; 2] = [196.0, 32.0];

/// How far each step scrolls, in pixels.
const STEP: f32 = 40.0;

/// A page whose port is the output less a scrollbar gutter, drawn over an opaque background
/// with a scrollbar and an empty overlay in a fixed space.
///
/// The content holds a node per row of panels, a node per panel with the panel's background,
/// and a node per box holding a chunk with a quad and a label. Some boxes are faded groups.
pub(crate) struct Page {
    compositor: CompositorImpl,
    client: ClientId,
    output: OutputId,
    frame: Frame,
    scroll: ScrollId,
    thumb: ChunkKey,
    offset: f32,
    step: f32,
}

impl Page {
    /// Returns the page past its first frame.
    pub(crate) fn new() -> Self {
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
        let port_width = width - GUTTER;
        let scroll = tx.create_scroll(ScrollNode {
            space,
            port: [0.0, 0.0, port_width, height],
            content: [port_width, content_height()],
            offset: [0.0, 0.0],
            behaviour: None,
        });
        let content = tx.create_transform(node(Some(space), Some(scroll)));
        let mut fixed = node(Some(content), None);
        fixed.anchor = Anchor::Fixed;
        let fixed = tx.create_transform(fixed);
        let port = tx.create_clip(ClipNode {
            parent: None,
            space,
            shape: ClipShape::RoundedRect {
                rect: [0.0, 0.0, port_width, height],
                radii: [0.0; 8],
                shapes: ROUND,
            },
        });
        let white = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));
        let card = tx.create_paint(Paint::Solid(Color([0.96, 0.96, 0.98, 1.0])));
        let grey = tx.create_paint(Paint::Solid(Color([0.8, 0.8, 0.84, 1.0])));
        let black = tx.create_paint(Paint::Solid(Color([0.0, 0.0, 0.0, 1.0])));
        let faded = tx.create_effect(EffectNode {
            clip: Some(port),
            opacity: 0.5,
            blend: BlendMode::NORMAL,
            filters: Vec::new().into(),
            isolate: false,
        });

        let background = ChunkKey::new(u64::MAX, 0);
        let mut chunk = tx.record(background, props(space, None));
        let fill = chunk.paint_ref(PaintKind::SOLID, white);
        chunk.quad(quad([0.0, 0.0, width, height], fill));
        let track = ChunkKey::new(u64::MAX, 1);
        let mut chunk = tx.record(track, props(space, None));
        let fill = chunk.paint_ref(PaintKind::SOLID, card);
        chunk.quad(quad([port_width, 0.0, GUTTER, height], fill));
        let thumb = ChunkKey::new(u64::MAX, 2);
        let mut chunk = tx.record(thumb, props(space, None));
        let fill = chunk.paint_ref(PaintKind::SOLID, grey);
        chunk.quad(quad([port_width + 3.0, 0.0, GUTTER - 6.0, 80.0], fill));

        let page = tx.create_order(OrderNode::default());
        let scrolled = tx.create_order(OrderNode::default());
        let overlay = tx.create_order(OrderNode::default());
        let empty = ChunkKey::new(u64::MAX, 3);
        tx.record(empty, props(fixed, None));
        tx.splice(overlay, 0, 0, &[OrderEntry::Chunk(empty)]);
        tx.splice(
            page,
            0,
            0,
            &[
                OrderEntry::Chunk(background),
                OrderEntry::Chunk(track),
                OrderEntry::Chunk(thumb),
                OrderEntry::Node(scrolled),
                OrderEntry::Node(overlay),
            ],
        );
        tx.splice(root, 0, 0, &[OrderEntry::Node(page)]);

        let mut rows = Vec::new();
        for first in (0..PANELS).step_by(3) {
            let row = tx.create_order(OrderNode::default());
            let mut panels = Vec::new();
            for panel in first..(first + 3).min(PANELS) {
                let at = [
                    24.0 + (panel % 3) as f32 * (PANEL[0] + 16.0),
                    24.0 + (panel / 3) as f32 * (PANEL[1] + 16.0),
                ];
                let id = tx.create_order(OrderNode::default());
                let key = ChunkKey::new(panel, 0);
                let mut chunk = tx.record(key, props(content, Some(port)));
                let fill = chunk.paint_ref(PaintKind::SOLID, card);
                chunk.quad(quad([at[0], at[1], PANEL[0], PANEL[1]], fill));
                let mut entries = vec![OrderEntry::Chunk(key)];
                for index in 0..BOXES {
                    let number = panel * u64::from(BOXES) + u64::from(index);
                    let origin = [
                        at[0] + 16.0 + (index % 3) as f32 * PITCH[0],
                        at[1] + 16.0 + (index / 3) as f32 * PITCH[1],
                    ];
                    let effect = (number % FADED_EVERY == 0).then_some(faded);
                    let id = tx.create_order(OrderNode { effect });
                    let key = ChunkKey::new(panel, index + 1);
                    let mut chunk = tx.record(key, props(content, Some(port)));
                    let fill = chunk.paint_ref(PaintKind::SOLID, white);
                    chunk.quad(quad([origin[0], origin[1], BOX[0], BOX[1]], fill));
                    let ink = chunk.paint_ref(PaintKind::SOLID, black);
                    let pen = [origin[0] + 8.0, origin[1] + 17.0];
                    let glyphs = chunk.glyphs(&layout.line(&format!("Button {number}"), 12.0, pen));
                    chunk.glyph_run(run(font, glyphs, ink));
                    tx.splice(id, 0, 0, &[OrderEntry::Chunk(key)]);
                    entries.push(OrderEntry::Node(id));
                }
                tx.splice(id, 0, 0, &entries);
                panels.push(OrderEntry::Node(id));
            }
            tx.splice(row, 0, 0, &panels);
            rows.push(OrderEntry::Node(row));
        }
        tx.splice(scrolled, 0, 0, &rows);
        compositor.commit(tx).expect("the page commits");

        let mut page = Self {
            compositor,
            client,
            output,
            frame: Frame::default(),
            scroll,
            thumb,
            offset: 0.0,
            step: STEP,
        };
        assert_eq!(page.draw(), Built::Frame);
        page
    }

    /// Commits a scroll by one step, with the scrollbar's thumb moved along.
    pub(crate) fn commit(&mut self) {
        let end = content_height() - SIDE[1] as f32;
        if !(0.0..=end).contains(&(self.offset + self.step)) {
            self.step = -self.step;
        }
        self.offset += self.step;
        let thumb = (self.offset / end * (SIDE[1] as f32 - 80.0)).round();

        let mut tx = self.compositor.transaction(self.client);
        tx.scroll_to(self.scroll, [0.0, self.offset]);
        tx.set_offset(self.thumb, [0.0, thumb]);
        self.compositor.commit(tx).expect("the scroll commits");
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

    /// Prints the work counters of one scroll's commit and the counters of its frame.
    pub(crate) fn report(&mut self, name: &str) {
        let before = self.stats();
        self.commit();
        let commit = self.stats();
        assert_eq!(self.draw(), Built::Frame);
        let frame = self.stats();
        let counters = self.compositor.counters().frame;

        let delta = |from: &[(&'static str, u64)], to: &[(&'static str, u64)]| {
            to.iter()
                .zip(from)
                .filter(|((_, after), (_, before))| after != before)
                .map(|((name, after), (_, before))| format!("{name} {}", after - before))
                .collect::<Vec<_>>()
                .join(", ")
        };
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

    /// Returns the compositor's work counters and shift refusals.
    fn stats(&self) -> Vec<(&'static str, u64)> {
        let mut stats = Vec::new();
        self.compositor.extra_stats(&mut |name, value| {
            if name.starts_with("work.") || name.starts_with("shift.") {
                stats.push((name, value));
            }
        });
        stats
    }
}

/// Returns the height of the page's content.
fn content_height() -> f32 {
    let rows = PANELS.div_ceil(3) as f32;
    24.0 + rows * (PANEL[1] + 16.0)
}
