//! iceblink-core with real fonts, drawing one scene on a target.

use std::sync::Arc;

use iceblink::damage::DamageSet;
use iceblink::{
    Anchor, BorderStyle, Built, ChunkFlags, ChunkKey, ChunkProps, ClientId, ClipRef, Color, Frame,
    FrameClock, FrameOutcome, FrameRequest, Matrix4, OrderEntry, OrderId, OrderNode, OutputId,
    Paint, PaintKind, PaintRef, Quad, ROUND, RenderCapabilities, SpaceRef, SubpixelOrder,
    Transaction, TransformId, TransformNode,
};
use iceblink::{Compositor as _, OutputConfig};
use iceblink_core::CompositorImpl;
use iceblink_testing::conformance::draw::DrawTarget;
use iceblink_testing::golden::RgbaImage;
use iceblink_testing::validate::{FrameValidator, describe};
use iceblink_text::SwashRaster;

/// The side of the output, in pixels.
pub(crate) const SIDE: u32 = 128;

/// The ids a scene is written against.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ids {
    /// The root order node, whose sequence the scene's chunks are appended to.
    pub(crate) root: OrderId,
    /// The root transform.
    pub(crate) space: TransformId,
}

impl Ids {
    /// Chunk props in the root space with no clip.
    pub(crate) fn props(self) -> ChunkProps {
        ChunkProps {
            space: self.space,
            clip: None,
            flags: ChunkFlags::default(),
        }
    }
}

/// iceblink-core with the swash glyph rasteriser, one client and one output, whose frames a
/// target draws.
pub(crate) struct Stage {
    pub(crate) compositor: CompositorImpl,
    target: Box<dyn DrawTarget>,
    client: ClientId,
    output: OutputId,
    pub(crate) ids: Ids,
    frame: Frame,
    validator: FrameValidator,
    caps: RenderCapabilities,
}

impl Stage {
    /// Returns a stage drawing into `target`, whose frames are built for `caps` with subpixel
    /// text in red to blue order.
    pub(crate) fn new(target: Box<dyn DrawTarget>, caps: RenderCapabilities) -> Self {
        let mut compositor = CompositorImpl::new();
        compositor.use_glyph_raster(Arc::new(SwashRaster::new()));
        let (client, _) = compositor.register_client();
        let output = compositor.create_output(OutputConfig {
            size: [SIDE, SIDE],
            scale: 1.0,
            opaque: true,
        });
        let mut tx = compositor.transaction(client);
        let space = tx.create_transform(TransformNode {
            parent: None,
            local: Matrix4::IDENTITY,
            scroll: None,
            anchor: Anchor::Scrolls,
        });
        let root = tx.create_order(OrderNode::default());
        tx.set_root(output, Some(root));
        compositor.commit(tx).expect("the setup commits");
        Self {
            compositor,
            target,
            client,
            output,
            ids: Ids { root, space },
            frame: Frame::default(),
            validator: FrameValidator::new(),
            caps,
        }
    }

    /// Commits what `write` writes into a transaction of the stage's client.
    pub(crate) fn commit(&mut self, write: impl FnOnce(&mut Transaction, Ids)) {
        let mut tx = self.compositor.transaction(self.client);
        write(&mut tx, self.ids);
        self.compositor.commit(tx).expect("the scene commits");
    }

    /// Builds and draws the next frame, and returns the image the target presents.
    ///
    /// # Panics
    ///
    /// Panics when a frame breaks the frame contract or is not presented.
    pub(crate) fn draw(&mut self) -> RgbaImage {
        self.validator.set_capabilities(Some(self.caps));
        let request = FrameRequest {
            caps: &self.caps,
            age: self.target.renderer().target_age(),
            clock: FrameClock::default(),
            subpixel: Some(SubpixelOrder::RedToBlue),
        };
        let built = self
            .compositor
            .build_frame(self.output, &request, &mut self.frame);
        if built == Built::Frame {
            if let Err(violations) = self.validator.check(&self.frame) {
                panic!(
                    "the frame breaks the frame contract:\n{}",
                    describe(&violations)
                );
            }
            let outcome = self.target.renderer().draw(&self.frame);
            assert!(
                matches!(outcome, FrameOutcome::Presented(_)),
                "the target did not present the frame: {outcome:?}"
            );
            self.compositor
                .frame_done(self.output, self.frame.id, &outcome);
        }
        self.target.presented()
    }

    /// Returns the output the stage draws.
    pub(crate) fn output(&self) -> OutputId {
        self.output
    }

    /// Returns the draw damage of the last frame built.
    pub(crate) fn damage(&self) -> &DamageSet {
        &self.frame.damage.draw
    }
}

/// Appends `chunks` to the root's sequence, which holds `held` entries.
pub(crate) fn append(tx: &mut Transaction, ids: Ids, held: u32, chunks: &[ChunkKey]) {
    let entries: Vec<OrderEntry> = chunks.iter().map(|key| OrderEntry::Chunk(*key)).collect();
    tx.splice(ids.root, held, 0, &entries);
}

/// Returns the key of chunk `owner`.
pub(crate) fn key(owner: u64) -> ChunkKey {
    ChunkKey::new(owner, 0)
}

/// Records chunk 1, a white quad over the output, and makes it the root's first entry.
pub(crate) fn background(tx: &mut Transaction, ids: Ids) {
    let white = tx.create_paint(Paint::Solid(Color([1.0; 4])));
    let mut chunk = tx.record(key(1), ids.props());
    let fill = chunk.paint_ref(PaintKind::SOLID, white);
    chunk.quad(Quad {
        bounds: [0.0, 0.0, SIDE as f32, SIDE as f32],
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
    });
    append(tx, ids, 0, &[key(1)]);
}

/// Returns the capabilities both renderers report.
pub(crate) fn common(a: RenderCapabilities, b: RenderCapabilities) -> RenderCapabilities {
    RenderCapabilities {
        max_texture_size: a.max_texture_size.min(b.max_texture_size),
        subpixel_text: a.subpixel_text && b.subpixel_text,
        custom_shaders: a.custom_shaders && b.custom_shaders,
        vector_compute: a.vector_compute && b.vector_compute,
        can_shift: a.can_shift && b.can_shift,
        resident_chunks: a.resident_chunks && b.resident_chunks,
        blend_modes: a.blend_modes & b.blend_modes,
        kept_rasters: a.kept_rasters && b.kept_rasters,
    }
}
