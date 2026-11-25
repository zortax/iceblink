//! A compositor with one client and one output, driven at chosen present times.

use iceblink::{
    Anchor, BorderStyle, BufferAge, Built, ChunkFlags, ChunkKey, ChunkProps, ClientId, ClipRef,
    Color, CompositorEvent, Frame, FrameClock, FrameOutcome, FrameRequest, FrameStats,
    GesturePhase, Matrix4, OrderEntry, OrderId, OrderNode, OutputId, Paint, PaintKind, PaintRef,
    Quad, ROUND, RenderCapabilities, ScrollGesture, ScrollId, ScrollPhase, ScrollRouting,
    ScrollSource, SpaceRef, Transaction, TransformId, TransformNode,
};
use iceblink::{Compositor, OutputConfig};

use crate::validate::{FrameValidator, describe};

/// Builds the compositor a rule is checked on.
pub type Factory<'a> = &'a dyn Fn() -> Box<dyn Compositor>;

/// The refresh interval of the frames a stage builds unless it is set otherwise, in nanoseconds.
pub const FRAME_NS: u64 = 16_666_667;

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

/// A compositor with one client and one opaque 200 by 200 output, whose root order node draws
/// under an identity transform.
pub struct Stage {
    /// The compositor under test.
    pub compositor: Box<dyn Compositor>,
    /// The only client.
    pub client: ClientId,
    /// The only output.
    pub output: OutputId,
    /// The root order node of the output.
    pub root: OrderId,
    /// The identity transform node at the root.
    pub space: TransformId,
    /// The last frame built.
    pub frame: Frame,
    /// The refresh interval of the frames the stage builds, in nanoseconds.
    pub interval_ns: u64,
    entries: u32,
    validator: FrameValidator,
}

impl Stage {
    /// Returns a stage past its first frame, presented at time zero.
    pub fn new(factory: Factory<'_>) -> Self {
        let mut compositor = factory();
        let (client, _) = compositor.register_client();
        let output = compositor.create_output(OutputConfig {
            size: [200, 200],
            scale: 1.0,
            opaque: true,
        });
        let mut tx = compositor.transaction(client);
        let space = tx.create_transform(identity(None));
        let root = tx.create_order(OrderNode::default());
        tx.set_root(output, Some(root));
        compositor.commit(tx).expect("the setup commits");

        let mut validator = FrameValidator::new();
        validator.set_capabilities(Some(CAPS));
        let mut stage = Self {
            compositor,
            client,
            output,
            root,
            space,
            frame: Frame::default(),
            interval_ns: FRAME_NS,
            entries: 0,
            validator,
        };
        stage.draw_at(0);
        stage
    }

    /// Returns a transaction of the stage's client.
    pub fn tx(&mut self) -> Transaction {
        self.compositor.transaction(self.client)
    }

    /// Commits a transaction.
    ///
    /// # Panics
    ///
    /// Panics when the commit is rejected.
    pub fn commit(&mut self, tx: Transaction) {
        self.compositor.commit(tx).expect("the transaction commits");
    }

    /// Appends an entry to the root's sequence in `tx`.
    pub fn push(&mut self, tx: &mut Transaction, entry: OrderEntry) {
        tx.splice(self.root, self.entries, 0, &[entry]);
        self.entries += 1;
    }

    /// Builds the frame presented at `present_ns` for a kept target, reports it presented, and
    /// returns what building returned.
    ///
    /// # Panics
    ///
    /// Panics when the frame breaks the frame contract.
    pub fn draw_at(&mut self, present_ns: u64) -> Built {
        let request = FrameRequest {
            caps: &CAPS,
            age: BufferAge::Kept,
            clock: FrameClock {
                now_ns: present_ns.saturating_sub(self.interval_ns),
                present_ns,
                interval_ns: self.interval_ns,
            },
            subpixel: None,
        };
        let built = self
            .compositor
            .build_frame(self.output, &request, &mut self.frame);
        if built == Built::Frame {
            if let Err(violations) = self.validator.check(&self.frame) {
                panic!(
                    "a frame breaks the frame contract:\n{}",
                    describe(&violations)
                );
            }
            let id = self.frame.id;
            self.compositor.frame_done(
                self.output,
                id,
                &FrameOutcome::Presented(FrameStats::default()),
            );
        }
        built
    }

    /// Drains the queued events.
    pub fn events(&mut self) -> Vec<CompositorEvent> {
        let mut events = Vec::new();
        self.compositor.drain_events(&mut events);
        events
    }

    /// Hands one step of a gesture at `position` to the compositor, and returns how it was routed.
    ///
    /// A `Step` comes from a wheel, and every other phase from a touchpad.
    pub fn gesture(
        &mut self,
        position: [f32; 2],
        delta: [f32; 2],
        phase: GesturePhase,
        time_ns: u64,
    ) -> ScrollRouting {
        let source = match phase {
            GesturePhase::Step => ScrollSource::Wheel,
            _ => ScrollSource::Touchpad,
        };
        self.compositor.scroll_input(
            self.output,
            ScrollGesture {
                position,
                delta,
                phase,
                source,
                time_ns,
            },
        )
    }

    /// Returns the offset, overscroll, serial and phase of the last scroll event of `node` in
    /// `events`.
    pub fn last_scroll(
        events: &[CompositorEvent],
        node: ScrollId,
    ) -> Option<([f32; 2], [f32; 2], u64, ScrollPhase)> {
        events.iter().rev().find_map(|event| match *event {
            CompositorEvent::Scroll {
                node: of,
                offset,
                overscroll,
                serial,
                phase,
                ..
            } if of == node => Some((offset, overscroll, serial, phase)),
            _ => None,
        })
    }
}

/// A transform node with an identity matrix under `parent`.
pub(crate) fn identity(parent: Option<TransformId>) -> TransformNode {
    TransformNode {
        parent,
        local: Matrix4::IDENTITY,
        scroll: None,
        anchor: Anchor::Scrolls,
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
