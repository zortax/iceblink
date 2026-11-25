//! A compositor wired to a renderer, driven through stories of transactions.

use std::sync::Arc;

use iceblink::{
    BufferAge, Built, ClientId, Frame, FrameClock, FrameOutcome, FrameRequest, OrderNode, OutputId,
    RenderCapabilities, RenderTarget, SubpixelOrder, Transaction,
};
use iceblink::{Compositor, OutputConfig, Renderer};
use iceblink_core::CompositorImpl;
use iceblink_testing::FrameValidator;
use iceblink_testing::validate::describe;
use iceblink_text::SwashRaster;

use crate::common::checked;
use crate::common::story::{Step, Story, World};
use crate::common::text::test_font;

/// The side of every slice's output, in pixels.
pub(crate) const SIDE: u32 = 128;

/// A compositor with one client and one output, whose frames a renderer draws.
pub(crate) struct Slice<R: Renderer> {
    pub(crate) compositor: CompositorImpl,
    pub(crate) renderer: R,
    pub(crate) client: ClientId,
    pub(crate) output: OutputId,
    pub(crate) world: World,
    pub(crate) frame: Frame,
    /// Follows what the renderer holds, and checks every frame built against it.
    pub(crate) validator: FrameValidator,
    /// The frames built so far.
    pub(crate) frames: u32,
    /// The stripe order frames are asked for.
    pub(crate) subpixel: Option<SubpixelOrder>,
    /// The clock frames are asked for.
    pub(crate) clock: FrameClock,
}

impl<R: Renderer> Slice<R> {
    /// Returns a slice drawing into `renderer`, with a glyph rasteriser, the test font, a root
    /// order node and a root transform, before its first frame.
    pub(crate) fn new(mut renderer: R) -> Self {
        let mut compositor = checked();
        compositor.use_glyph_raster(Arc::new(SwashRaster::new()));
        let font = compositor.font_registry().register(test_font());
        let (client, _) = compositor.register_client();
        let config = OutputConfig {
            size: [SIDE, SIDE],
            scale: 1.0,
            opaque: true,
        };
        let output = compositor.create_output(config);

        renderer.configure(RenderTarget {
            size: config.size,
            scale: config.scale,
            opaque: config.opaque,
        });

        let mut tx = compositor.transaction(client);
        let space = tx.create_transform(crate::common::identity(None));
        let root = tx.create_order(OrderNode::default());
        tx.set_root(output, Some(root));
        compositor.commit(tx).expect("the setup commits");

        let mut validator = FrameValidator::new();
        validator.set_capabilities(Some(renderer.capabilities()));
        Self {
            compositor,
            renderer,
            client,
            output,
            world: World::new(root, space, font),
            frame: Frame::default(),
            validator,
            frames: 0,
            subpixel: None,
            clock: FrameClock::default(),
        }
    }

    /// Returns a transaction of the slice's client.
    pub(crate) fn tx(&mut self) -> Transaction {
        self.compositor.transaction(self.client)
    }

    /// Commits one step of a story.
    pub(crate) fn apply(&mut self, step: Step) {
        let mut tx = self.tx();
        step(&mut tx, &mut self.world);
        self.compositor.commit(tx).expect("the step commits");
    }

    /// Commits every step of a story in one transaction.
    pub(crate) fn apply_all(&mut self, story: &Story) {
        let mut tx = self.tx();
        for step in story.steps {
            step(&mut tx, &mut self.world);
        }
        self.compositor.commit(tx).expect("the story commits");
    }

    /// Builds a frame for the renderer's target, draws it and reports the outcome.
    ///
    /// Returns whether a frame was built.
    pub(crate) fn draw(&mut self) -> bool {
        let caps = self.renderer.capabilities();
        self.draw_with(caps)
    }

    /// Builds a frame for `caps` and the renderer's target, draws it and reports the outcome.
    ///
    /// Returns whether a frame was built.
    pub(crate) fn draw_with(&mut self, caps: RenderCapabilities) -> bool {
        let request = FrameRequest {
            caps: &caps,
            age: self.renderer.target_age(),
            clock: self.clock,
            subpixel: self.subpixel,
        };
        let built = self
            .compositor
            .build_frame(self.output, &request, &mut self.frame);
        if built != Built::Frame {
            return false;
        }

        self.frames += 1;
        if let Err(violations) = self.validator.check(&self.frame) {
            panic!(
                "frame {} breaks the frame contract:\n{}",
                self.frame.id.0,
                describe(&violations)
            );
        }

        let outcome = self.renderer.draw(&self.frame);
        if outcome == FrameOutcome::Recovered {
            self.validator.device_lost();
        }
        self.compositor
            .frame_done(self.output, self.frame.id, &outcome);
        true
    }

    /// Builds and draws a frame that redraws the whole output from the retained state.
    pub(crate) fn redraw_all(&mut self) {
        // A change of the renderer's capabilities damages the whole output and changes no
        // lowering.
        let mut caps = self.renderer.capabilities();
        caps.blend_modes += 1;
        self.validator.set_capabilities(Some(caps));
        assert!(self.draw_with(caps), "a capability change builds a frame");
        assert!(self.frame.damage.draw.is_full());
    }

    /// Plays a story one step per frame.
    pub(crate) fn play(&mut self, story: &Story) {
        for step in story.steps {
            self.apply(*step);
            self.draw();
        }
    }

    /// Builds a frame for a kept target without drawing it, and returns what building returned.
    pub(crate) fn build_only(&mut self) -> Built {
        let caps = self.renderer.capabilities();
        let request = FrameRequest {
            caps: &caps,
            age: BufferAge::Kept,
            clock: FrameClock::default(),
            subpixel: None,
        };
        self.compositor
            .build_frame(self.output, &request, &mut self.frame)
    }
}
