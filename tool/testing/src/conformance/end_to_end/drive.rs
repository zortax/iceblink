//! One compositor drawing into one target, frame by frame.

use std::sync::Arc;

use iceblink::{
    BufferAge, Built, ClientId, CompositorEvent, Frame, FrameClock, FrameOutcome, FrameRequest,
    OutputId, RenderCapabilities, ScrollGesture, SkipReason, Transaction,
};
use iceblink::{Compositor, OutputConfig};

use crate::conformance::draw::DrawTarget;
use crate::conformance::end_to_end::Pair;
use crate::fakes::{FakeFontRegistry, FakeGlyphRaster, fake_font};
use crate::golden::RgbaImage;
use crate::validate::{FrameValidator, describe};

/// The side of the output, in pixels.
pub(crate) const SIDE: u32 = 128;

/// A compositor with one client and one output, whose frames a target draws.
pub(crate) struct Drive {
    compositor: Box<dyn Compositor>,
    target: Box<dyn DrawTarget>,
    client: ClientId,
    output: OutputId,
    frame: Frame,
    validator: FrameValidator,
    /// Whether every frame redraws the whole output from the retained scene.
    whole: bool,
    /// The capabilities frames are requested with, when not the renderer's.
    caps: Option<RenderCapabilities>,
    /// The clock frames are requested with.
    clock: FrameClock,
}

impl Drive {
    /// Returns a drive of a compositor and a target from the factories, or `None` when the
    /// target factory has no target.
    ///
    /// A drive that draws `whole` forgets the compositor's caches before every frame, so every
    /// frame redraws the whole output.
    pub(crate) fn new(pair: Pair<'_>, whole: bool) -> Option<Self> {
        let target = (pair.target)([SIDE, SIDE])?;

        let mut compositor = (pair.compositor)();
        compositor.use_font_registry(Arc::new(FakeFontRegistry::new()));
        compositor.use_glyph_raster(Arc::new(FakeGlyphRaster::new()));
        compositor.font_registry().register(fake_font(1));

        let (client, _) = compositor.register_client();
        let output = compositor.create_output(OutputConfig {
            size: [SIDE, SIDE],
            scale: 1.0,
            opaque: true,
        });
        Some(Self {
            compositor,
            target,
            client,
            output,
            frame: Frame::default(),
            validator: FrameValidator::new(),
            whole,
            caps: pair.caps,
            clock: FrameClock::default(),
        })
    }

    /// Returns the blend modes the drive's frames are drawn with, as a bit per `BlendMode` value.
    pub(crate) fn blend_modes(&mut self) -> u32 {
        self.caps
            .unwrap_or_else(|| self.target.renderer().capabilities())
            .blend_modes
    }

    /// Returns the output frames are built for.
    pub(crate) fn output(&self) -> OutputId {
        self.output
    }

    /// Returns the compositor under test.
    pub(crate) fn compositor(&mut self) -> &mut dyn Compositor {
        self.compositor.as_mut()
    }

    /// Sets the time the next frames are presented at, with frames `interval_ns` apart.
    pub(crate) fn at(&mut self, present_ns: u64, interval_ns: u64) {
        self.clock = FrameClock {
            now_ns: present_ns.saturating_sub(interval_ns),
            present_ns,
            interval_ns,
        };
    }

    /// Sends one step of a scroll gesture to the compositor.
    pub(crate) fn scroll_input(&mut self, gesture: ScrollGesture) {
        self.compositor.scroll_input(self.output, gesture);
    }

    /// Drains the compositor's events.
    pub(crate) fn events(&mut self) -> Vec<CompositorEvent> {
        let mut events = Vec::new();
        self.compositor.drain_events(&mut events);
        events
    }

    /// Commits what `write` writes into a transaction of the drive's client.
    ///
    /// # Panics
    ///
    /// Panics when the commit is rejected.
    #[track_caller]
    pub(crate) fn commit(&mut self, write: impl FnOnce(&mut Transaction)) {
        let mut tx = self.compositor.transaction(self.client);
        write(&mut tx);
        if let Err(error) = self.compositor.commit(tx) {
            panic!("a scene transaction was rejected: {error}");
        }
    }

    /// Builds and draws the next frame, and returns the image the target presents.
    ///
    /// # Panics
    ///
    /// Panics when the frame breaks the frame contract, or when the renderer neither presents it
    /// nor skips it as undamaged.
    #[track_caller]
    pub(crate) fn draw(&mut self) -> RgbaImage {
        let caps = self
            .caps
            .unwrap_or_else(|| self.target.renderer().capabilities());
        self.validator.set_capabilities(Some(caps));

        let age = if self.whole {
            self.compositor.forget_caches();
            BufferAge::Unknown
        } else {
            self.target.renderer().target_age()
        };
        let request = FrameRequest {
            caps: &caps,
            age,
            clock: self.clock,
            subpixel: None,
        };
        let built = self
            .compositor
            .build_frame(self.output, &request, &mut self.frame);
        if built == Built::Frame {
            self.render();
        }
        self.target.presented()
    }

    /// Validates the built frame, draws it on the target and reports the outcome to the
    /// compositor.
    #[track_caller]
    fn render(&mut self) {
        if let Err(violations) = self.validator.check(&self.frame) {
            panic!(
                "frame {} breaks the frame contract:\n{}",
                self.frame.id.0,
                describe(&violations)
            );
        }

        let outcome = self.target.renderer().draw(&self.frame);
        if outcome == FrameOutcome::Recovered {
            self.validator.device_lost();
        }
        assert!(
            outcome.stats().is_some() || outcome == FrameOutcome::Skipped(SkipReason::Undamaged),
            "the target did not present the frame: {outcome:?}"
        );
        self.compositor
            .frame_done(self.output, self.frame.id, &outcome);
    }
}
