//! A rig configured for a case, and the frames the cases draw on it.

use iceblink::{Frame, FrameId, FrameOutcome, PaintRef, PresentMode, RenderTarget};

use crate::conformance::draw::builder::{FrameBuilder, filled, opaque};
use crate::conformance::present::{MakeRig, PresentRig};
use crate::golden::RgbaImage;
use crate::validate::{FrameValidator, describe};

/// The side of every case's output, in pixels.
pub(crate) const SIDE: u32 = 64;

/// A rig for a target of `SIDE` by `SIDE` pixels, which validates every frame it draws.
pub(crate) struct Stand {
    pub(crate) rig: Box<dyn PresentRig>,
    validator: FrameValidator,
    frames: u64,
}

impl Stand {
    /// Returns an unconfigured rig in `mode`, or `None` after reporting the skip.
    pub(crate) fn unconfigured(make: MakeRig<'_>, mode: PresentMode, case: &str) -> Option<Self> {
        let Some(rig) = make(mode, [SIDE, SIDE]) else {
            skip(case, &format!("the factory builds no rig in {mode:?} mode"));
            return None;
        };
        Some(Self {
            rig,
            validator: FrameValidator::new(),
            frames: 0,
        })
    }

    /// Returns a rig in `mode` configured for the case's target, or `None` after reporting the
    /// skip.
    pub(crate) fn open(make: MakeRig<'_>, mode: PresentMode, case: &str) -> Option<Self> {
        let mut stand = Self::unconfigured(make, mode, case)?;
        stand.configure();
        Some(stand)
    }

    /// Configures the renderer for the case's target.
    pub(crate) fn configure(&mut self) {
        self.rig.renderer().configure(RenderTarget {
            size: [SIDE, SIDE],
            scale: 1.0,
            opaque: true,
        });
    }

    /// Draws `frame` and returns the outcome.
    ///
    /// # Panics
    ///
    /// Panics when the frame breaks the frame contract.
    #[track_caller]
    pub(crate) fn draw(&mut self, mut frame: Frame) -> FrameOutcome {
        self.frames += 1;
        frame.id = FrameId(self.frames);

        if let Err(violations) = self.validator.check(&frame) {
            panic!(
                "the case's frame breaks the frame contract:\n{}",
                describe(&violations)
            );
        }

        let outcome = self.rig.renderer().draw(&frame);
        if outcome == FrameOutcome::Recovered {
            self.validator.device_lost();
        }
        outcome
    }

    /// Reads back the image presented last.
    ///
    /// # Panics
    ///
    /// Panics when nothing was presented.
    #[track_caller]
    pub(crate) fn presented(&mut self) -> RgbaImage {
        self.rig.presented().expect("an image was presented")
    }
}

/// Reports a case skipped because the rig cannot provoke what it checks.
pub(crate) fn skip(case: &str, reason: &str) {
    eprintln!("skipped: present case {case}: {reason}");
}

/// Returns a first frame filled with `color` over the whole output, and the paint it uses.
pub(crate) fn filled_frame(color: [u8; 3]) -> (Frame, PaintRef) {
    let mut builder = FrameBuilder::new([SIDE, SIDE]);
    let paint = builder.solid(opaque(color[0], color[1], color[2]));
    builder.quad(filled([0.0, 0.0, SIDE as f32, SIDE as f32], paint));
    (builder.finish(), paint)
}

/// Returns the colour of the centre pixel of `image`.
pub(crate) fn centre(image: &RgbaImage) -> [u8; 4] {
    image.pixel(SIDE / 2, SIDE / 2)
}
