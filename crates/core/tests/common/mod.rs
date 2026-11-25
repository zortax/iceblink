//! Builders, fakes and fixtures shared by the iceblink-core tests.

// Every test binary compiles this module and uses a subset of it.
#![allow(dead_code)]

pub(crate) mod canon;
pub(crate) mod content;
pub(crate) mod gpu;
pub(crate) mod list;
pub(crate) mod numbers;
pub(crate) mod plan;
pub(crate) mod slice;
pub(crate) mod stories;
pub(crate) mod story;
pub(crate) mod text;
pub(crate) mod twin;

use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    Anchor, BorderStyle, BufferAge, Built, ChunkFlags, ChunkKey, ChunkProps, ClientId, ClipRef,
    Color, DeviceRect, Frame, FrameClock, FrameOutcome, FrameRequest, FrameStats, IdSpace, Matrix4,
    OrderEntry, OrderId, OrderNode, OutputId, Paint, PaintKind, PaintRef, Quad, ROUND,
    RenderCapabilities, SpaceRef, Transaction, TransformId, TransformNode,
};
use iceblink::{Compositor, OutputConfig};
use iceblink_core::CompositorImpl;
use iceblink_testing::validate::describe;
use iceblink_testing::{FrameValidator, validate_frame};

/// The capabilities of a renderer that shifts, keeps chunks resident and runs custom shaders.
pub(crate) const FULL: RenderCapabilities = RenderCapabilities {
    max_texture_size: 8192,
    subpixel_text: false,
    custom_shaders: true,
    vector_compute: false,
    can_shift: true,
    resident_chunks: true,
    blend_modes: 1,
    kept_rasters: false,
};

/// A compositor with one client, one 200 by 200 output and a root order node under an identity
/// transform, past its first frame.
pub(crate) struct Scene {
    pub(crate) compositor: CompositorImpl,
    pub(crate) client: ClientId,
    pub(crate) ids: IdSpace,
    pub(crate) output: OutputId,
    pub(crate) root: OrderId,
    pub(crate) space: TransformId,
    pub(crate) frame: Frame,
    pub(crate) caps: RenderCapabilities,
    /// How many entries the root's sequence holds.
    pub(crate) entries: u32,
    /// Follows what the output's renderer holds, and checks every frame built against it.
    pub(crate) validator: FrameValidator,
}

impl Scene {
    pub(crate) fn new() -> Self {
        Self::with_caps(FULL)
    }

    pub(crate) fn with_caps(caps: RenderCapabilities) -> Self {
        let mut compositor = checked();
        let (client, ids) = compositor.register_client();
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

        let mut scene = Self {
            compositor,
            client,
            ids,
            output,
            root,
            space,
            frame: Frame::default(),
            caps,
            entries: 0,
            validator: FrameValidator::new(),
        };
        scene.validator.set_capabilities(Some(caps));
        scene.draw();
        scene
    }

    pub(crate) fn tx(&mut self) -> Transaction {
        self.compositor.transaction(self.client)
    }

    pub(crate) fn commit(&mut self, tx: Transaction) {
        self.compositor.commit(tx).expect("the transaction commits");
    }

    /// Builds a frame for a kept target.
    pub(crate) fn build(&mut self) -> Built {
        self.build_aged(BufferAge::Kept)
    }

    pub(crate) fn build_aged(&mut self, age: BufferAge) -> Built {
        let request = FrameRequest {
            caps: &self.caps,
            age,
            clock: FrameClock::default(),
            subpixel: None,
        };
        let built = self
            .compositor
            .build_frame(self.output, &request, &mut self.frame);

        if built == Built::Frame
            && let Err(violations) = self.validator.check(&self.frame)
        {
            panic!(
                "frame {} breaks the frame contract:\n{}",
                self.frame.id.0,
                describe(&violations)
            );
        }
        built
    }

    /// Reports `outcome` for the last frame built.
    pub(crate) fn done(&mut self, outcome: FrameOutcome) {
        if outcome == FrameOutcome::Recovered {
            self.validator.device_lost();
        }
        let id = self.frame.id;
        self.compositor.frame_done(self.output, id, &outcome);
    }

    /// Builds a frame and reports it presented if one was built.
    ///
    /// Returns what building returned.
    pub(crate) fn draw(&mut self) -> Built {
        let built = self.build();
        if built == Built::Frame {
            self.done(presented());
        }
        built
    }

    /// Records chunk `key` with one quad per rectangle and appends it to the root.
    pub(crate) fn add_chunk(&mut self, key: ChunkKey, rects: &[[f32; 4]]) {
        let mut tx = self.tx();
        let paint = tx.create_paint(Paint::Solid(Color([1.0, 0.0, 0.0, 1.0])));
        let props = props(self.space);
        let mut chunk = tx.record(key, props);
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        for &rect in rects {
            chunk.quad(quad(rect, fill));
        }
        tx.splice(self.root, self.entries, 0, &[OrderEntry::Chunk(key)]);
        self.commit(tx);
        self.entries += 1;
    }

    /// Returns the frame's draw damage, or `None` when it is the whole output.
    pub(crate) fn damage(&self) -> Option<Vec<DeviceRect>> {
        let draw = &self.frame.damage.draw;
        (!draw.is_full()).then(|| draw.rects().to_vec())
    }
}

/// Returns a compositor in verification mode that checks every frame it builds against the frame
/// contract.
pub(crate) fn checked() -> CompositorImpl {
    let mut compositor = CompositorImpl::new();
    compositor.set_verification(true);
    compositor.set_frame_check(Some(Box::new(|frame| {
        validate_frame(frame).map_err(|violations| describe(&violations))
    })));
    compositor
}

/// Returns the value of an implementation counter of `compositor`, or zero when it reports none
/// by that name.
pub(crate) fn stat(compositor: &CompositorImpl, name: &str) -> u64 {
    let mut found = 0;
    compositor.extra_stats(&mut |stat, value| {
        if stat == name {
            found = value;
        }
    });
    found
}

/// Returns a transform node under `parent` with an identity matrix.
pub(crate) fn identity(parent: Option<TransformId>) -> TransformNode {
    TransformNode {
        parent,
        local: Matrix4::IDENTITY,
        scroll: None,
        anchor: Anchor::Scrolls,
    }
}

/// Returns chunk props in `space` with no clip.
pub(crate) fn props(space: TransformId) -> ChunkProps {
    ChunkProps {
        space,
        clip: None,
        flags: ChunkFlags::default(),
    }
}

/// Returns a borderless quad over `bounds` filled with `fill`.
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

pub(crate) fn rect(x: i32, y: i32, width: i32, height: i32) -> DeviceRect {
    Rect::new(Point::new(x, y), Size::new(width, height))
}

/// Returns a presented outcome with default frame stats.
pub(crate) fn presented() -> FrameOutcome {
    FrameOutcome::Presented(FrameStats::default())
}
