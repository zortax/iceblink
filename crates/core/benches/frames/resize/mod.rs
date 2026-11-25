//! A conversation in a scroll port, with the window dragged wider one step at a time.

mod panels;

use std::sync::Arc;
use std::time::{Duration, Instant};

use iceblink::{
    BufferAge, Built, ChunkKey, ClientId, ClipId, ClipNode, ClipShape, Color, Frame, FrameClock,
    FrameOutcome, FrameRequest, FrameStats, OrderEntry, OrderNode, OutputId, Paint, PaintId,
    PaintKind, ROUND, RenderCapabilities, ScrollId, ScrollNode, Transaction, TransformId,
};
use iceblink::{Compositor, OutputConfig};
use iceblink_core::CompositorImpl;
use iceblink_text::SwashRaster;

use self::panels::Panels;
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

/// The output's narrowest width in pixels.
const NARROWEST: u32 = 1280;

/// The output's height in pixels.
const HEIGHT: u32 = 1000;

/// How far each step widens the output.
const STEP: u32 = 8;

/// How many widths the steps cycle through.
const CYCLE: u32 = 24;

/// The width of the scrollbar gutter at the right edge of the output.
const GUTTER: f32 = 15.0;

/// The number of messages of the conversation.
const MESSAGES: u64 = 250;

/// The height of a line of text.
const LINE: f32 = 19.0;

/// The gap between messages.
const GAP: f32 = 10.0;

/// The words the paragraphs are made of.
const WORDS: &[&str] = &[
    "the",
    "resize",
    "asks",
    "every",
    "paragraph",
    "its",
    "question",
    "again",
    "and",
    "what",
    "one",
    "step",
    "costs",
    "is",
    "decided",
    "by",
    "how",
    "much",
    "of",
    "last",
];

/// A conversation drawn over an opaque background, with a scrollbar beside its port.
///
/// The port's content holds a node per message, with a chunk for the message's header and a node
/// per paragraph holding a chunk of three lines. Every tenth message carries a card. Every chunk
/// is drawn through the port clip, which lies inside the window clip. With panels, a column of
/// panels in the content follows the port's width.
pub(crate) struct Resize {
    compositor: CompositorImpl,
    client: ClientId,
    output: OutputId,
    frame: Frame,
    space: TransformId,
    scroll: ScrollId,
    window: ClipId,
    port: ClipId,
    track: ChunkKey,
    thumb: ChunkKey,
    grey: PaintId,
    content_height: f32,
    panels: Option<Panels>,
    step: u32,
}

impl Resize {
    /// Returns the conversation past its first frame.
    pub(crate) fn new() -> Self {
        Self::build(false)
    }

    /// Returns the conversation beside a column of panels, past its first frame.
    pub(crate) fn with_panels() -> Self {
        Self::build(true)
    }

    fn build(with_panels: bool) -> Self {
        let mut compositor = CompositorImpl::new();
        compositor.use_glyph_raster(Arc::new(SwashRaster::new()));
        let font = compositor
            .font_registry()
            .register(iceblink_testing::fonts::read_font(
                iceblink_testing::fonts::LATIN,
            ));
        let (client, _) = compositor.register_client();
        let output = compositor.create_output(OutputConfig {
            size: [NARROWEST, HEIGHT],
            scale: 1.0,
            opaque: true,
        });
        let layout = Layout::new();

        let mut tx = compositor.transaction(client);
        let space = tx.create_transform(node(None, None));
        let root = tx.create_order(OrderNode::default());
        tx.set_root(output, Some(root));
        let width = NARROWEST as f32;
        let window = tx.create_clip(window_clip(space, width));
        let port = tx.create_clip(port_clip(space, window, width));
        // The content's height is known once the messages are laid out.
        let scroll = tx.create_scroll(port_node(space, width, HEIGHT as f32));
        let content = tx.create_transform(node(Some(space), Some(scroll)));
        let canvas = tx.create_paint(Paint::Solid(Color([0.08, 0.09, 0.1, 1.0])));
        let card = tx.create_paint(Paint::Solid(Color([0.1, 0.12, 0.17, 1.0])));
        let grey = tx.create_paint(Paint::Solid(Color([0.5, 0.5, 0.55, 1.0])));
        let ink = tx.create_paint(Paint::Solid(Color([0.9, 0.92, 0.96, 1.0])));

        let background = ChunkKey::new(u64::MAX, 0);
        let mut chunk = tx.record(background, props(space, None));
        let fill = chunk.paint_ref(PaintKind::SOLID, canvas);
        chunk.quad(quad([0.0, 0.0, 4096.0, HEIGHT as f32], fill));
        let track = ChunkKey::new(u64::MAX, 1);
        let thumb = ChunkKey::new(u64::MAX, 2);
        let scrolled = tx.create_order(OrderNode::default());
        tx.splice(
            root,
            0,
            0,
            &[
                OrderEntry::Chunk(background),
                OrderEntry::Node(scrolled),
                OrderEntry::Chunk(track),
                OrderEntry::Chunk(thumb),
            ],
        );

        let mut messages = Vec::new();
        let mut y = 12.0;
        for message in 0..MESSAGES {
            let id = tx.create_order(OrderNode::default());
            let head = ChunkKey::new(message, 0);
            let mut chunk = tx.record(head, props(content, Some(port)));
            let fill = chunk.paint_ref(PaintKind::SOLID, ink);
            let name = format!("agent-{}", message % 7);
            let glyphs = chunk.glyphs(&layout.line(&name, 13.0, [12.0, y + 14.0]));
            chunk.glyph_run(run(font, glyphs, fill));
            y += LINE;
            let mut entries = vec![OrderEntry::Chunk(head)];
            for paragraph in 0..1 + message % 3 {
                let para = tx.create_order(OrderNode::default());
                let key = ChunkKey::new(message, 1 + paragraph as u32);
                let mut chunk = tx.record(key, props(content, Some(port)));
                let fill = chunk.paint_ref(PaintKind::SOLID, ink);
                for line in 0..3 {
                    let text = prose(message * 131 + paragraph * 17 + line, 9);
                    let glyphs = chunk.glyphs(&layout.line(&text, 13.0, [12.0, y + 14.0]));
                    chunk.glyph_run(run(font, glyphs, fill));
                    y += LINE;
                }
                tx.splice(para, 0, 0, &[OrderEntry::Chunk(key)]);
                entries.push(OrderEntry::Node(para));
            }
            if message.is_multiple_of(10) {
                let key = ChunkKey::new(message, 8);
                let mut chunk = tx.record(key, props(content, Some(port)));
                let fill = chunk.paint_ref(PaintKind::SOLID, card);
                chunk.quad(quad([12.0, y, 320.0, 80.0], fill));
                y += 80.0;
                entries.push(OrderEntry::Chunk(key));
            }
            y += GAP;
            tx.splice(id, 0, 0, &entries);
            messages.push(OrderEntry::Node(id));
        }
        tx.splice(scrolled, 0, 0, &messages);
        let panels = with_panels.then(|| {
            let at = messages.len() as u32;
            Panels::new(
                &mut tx,
                [space, content],
                port,
                (scrolled, at),
                width - GUTTER,
            )
        });

        let mut resize = Self {
            compositor,
            client,
            output,
            frame: Frame::default(),
            space,
            scroll,
            window,
            port,
            track,
            thumb,
            grey,
            content_height: y,
            panels,
            step: 0,
        };
        resize.scrollbar(&mut tx, width);
        tx.set_scroll_node(scroll, port_node(space, width, resize.content_height));
        resize
            .compositor
            .commit(tx)
            .expect("the conversation commits");
        assert_eq!(resize.draw(), Built::Frame);
        resize
    }

    /// Widens the output by one step and commits the port, the clips, the scrollbar and the
    /// panels that follow its width.
    pub(crate) fn commit(&mut self) {
        self.step += 1;
        let width = NARROWEST + (self.step % CYCLE) * STEP;
        self.compositor.configure_output(
            self.output,
            OutputConfig {
                size: [width, HEIGHT],
                scale: 1.0,
                opaque: true,
            },
        );

        let width = width as f32;
        let mut tx = self.compositor.transaction(self.client);
        tx.set_scroll_node(
            self.scroll,
            port_node(self.space, width, self.content_height),
        );
        tx.set_clip(self.window, window_clip(self.space, width));
        tx.set_clip(self.port, port_clip(self.space, self.window, width));
        self.scrollbar(&mut tx, width);
        if let Some(panels) = &self.panels {
            panels.record(&mut tx, width - GUTTER, self.step);
        }
        self.compositor.commit(tx).expect("the resize commits");
    }

    /// Records the scrollbar's track and thumb at the right edge of an output `width` wide.
    fn scrollbar(&self, tx: &mut Transaction, width: f32) {
        let mut chunk = tx.record(self.track, props(self.space, None));
        let fill = chunk.paint_ref(PaintKind::SOLID, self.grey);
        chunk.quad(quad([width - GUTTER, 0.0, GUTTER, HEIGHT as f32], fill));

        let mut chunk = tx.record(self.thumb, props(self.space, None));
        let fill = chunk.paint_ref(PaintKind::SOLID, self.grey);
        chunk.quad(quad([width - GUTTER + 3.0, 0.0, GUTTER - 6.0, 60.0], fill));
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

    /// Returns the time `iterations` resize commits take, with their frames built outside it.
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

    /// Returns the time the frames of `iterations` resizes take, with their commits outside it.
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

    /// Prints the work counters of one resize's commit and the counters of its frame.
    pub(crate) fn report(&mut self, name: &str) {
        let before = self.stats();
        self.commit();
        let commit = self.stats();
        assert_eq!(self.draw(), Built::Frame);
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
        println!(
            "{name}: frame: chunks_drawn {}, items_drawn {}, damage_px {}",
            counters.chunks_drawn, counters.items_drawn, counters.damage_px,
        );
    }

    /// Returns the compositor's work counters.
    fn stats(&self) -> Vec<(&'static str, u64)> {
        let mut stats = Vec::new();
        self.compositor.extra_stats(&mut |name, value| {
            if name.starts_with("work.") {
                stats.push((name, value));
            }
        });
        stats
    }
}

/// Returns the scroll node of a port beside the scrollbar of an output `width` wide.
fn port_node(space: TransformId, width: f32, content_height: f32) -> ScrollNode {
    ScrollNode {
        space,
        port: [0.0, 0.0, width - GUTTER, HEIGHT as f32],
        content: [width - GUTTER, content_height],
        offset: [0.0, 0.0],
        behaviour: None,
    }
}

/// Returns the clip of the whole window.
fn window_clip(space: TransformId, width: f32) -> ClipNode {
    rect_clip(None, space, [0.0, 0.0, width, HEIGHT as f32])
}

/// Returns the clip of the scroll port, inside the window clip.
fn port_clip(space: TransformId, window: ClipId, width: f32) -> ClipNode {
    rect_clip(
        Some(window),
        space,
        [0.0, 0.0, width - GUTTER, HEIGHT as f32],
    )
}

/// Returns a clip of `rect` under `parent`.
fn rect_clip(parent: Option<ClipId>, space: TransformId, rect: [f32; 4]) -> ClipNode {
    ClipNode {
        parent,
        space,
        shape: ClipShape::RoundedRect {
            rect,
            radii: [0.0; 8],
            shapes: ROUND,
        },
    }
}

/// Returns `count` words of prose chosen by `seed`.
fn prose(seed: u64, count: u64) -> String {
    (0..count)
        .map(|step| WORDS[(seed * 31 + step * 7) as usize % WORDS.len()])
        .collect::<Vec<_>>()
        .join(" ")
}
