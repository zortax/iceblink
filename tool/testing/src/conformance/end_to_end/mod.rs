//! The end-to-end suite: random scenes and edits drawn incrementally and whole, and one
//! transcript drawn by two compositor and renderer pairs.
//!
//! A scene is an opaque backing, a scroll port with its clip, a space that edits move, quads,
//! glyph runs and paths in chunks, and order nodes with opacity, blend, blur and drop-shadow
//! effects, one nested in another. Each step commits one to three random edits: a scroll, a move,
//! an effect or paint write, a splice, a removal, a recording, a chunk offset or a new port clip.
//! Every compositor gets a [`FakeGlyphRaster`](crate::fakes::FakeGlyphRaster), and every frame
//! passes the frame validator.

mod drive;
mod driven;
mod numbers;
mod random;
mod scene;

use iceblink::{RenderCapabilities, Renderer};

use crate::conformance::draw::{DrawTarget, MakeTarget};
use crate::conformance::end_to_end::drive::Drive;
use crate::conformance::end_to_end::random::{Player, SEEDS, STEPS, play};
use crate::conformance::end_to_end::scene::Model;
use crate::conformance::scene::MakeCompositor;
use crate::golden::{RgbaImage, Tolerance};

/// How far an incremental frame may differ from the same frame redrawn whole.
// Targets of another size round a blend differently, which costs a second step.
pub(crate) const INCREMENTAL: Tolerance = Tolerance {
    max_channel: 2,
    max_pixels: 0,
};

/// A compositor and the target its frames are drawn into.
#[derive(Clone, Copy)]
pub struct Pair<'a> {
    /// Builds the compositor.
    pub compositor: MakeCompositor<'a>,
    /// Builds the target.
    pub target: MakeTarget<'a>,
    /// The capabilities frames are requested with, in place of the renderer's own, or `None` for
    /// the renderer's. They must not claim a feature the renderer lacks.
    pub caps: Option<RenderCapabilities>,
}

/// A renderer under test whose images are not read back.
struct Blind(Box<dyn Renderer>);

impl DrawTarget for Blind {
    fn renderer(&mut self) -> &mut dyn Renderer {
        self.0.as_mut()
    }

    fn presented(&mut self) -> RgbaImage {
        RgbaImage::new(1, 1)
    }
}

/// Builds a renderer for a target of the given size, configured for it.
pub type MakeRenderer<'a> = &'a dyn Fn([u32; 2]) -> Box<dyn Renderer>;

/// Random scenes and edits from compositors of `compositor` are drawn by renderers of
/// `renderer`, which present every frame that has damage.
///
/// Every frame passes the frame validator before the renderer receives it.
pub fn frames_reach_a_renderer(compositor: MakeCompositor<'_>, renderer: MakeRenderer<'_>) {
    let target =
        |size: [u32; 2]| -> Option<Box<dyn DrawTarget>> { Some(Box::new(Blind(renderer(size)))) };
    let pair = Pair {
        compositor,
        target: &target,
        caps: None,
    };
    for seed in 0..SEEDS {
        let Some(drive) = Drive::new(pair, false) else {
            return;
        };
        let mut player = Player::new(drive);
        let blend_modes = player.drive.blend_modes();
        let mut model = Model::new(seed, blend_modes);

        for _ in 0..=STEPS {
            let edits = model.edits();
            player.step(&mut model, edits);
            player.drive.draw();
        }
    }
}

/// Random edits drawn incrementally into a kept target equal the same frames redrawn whole.
///
/// Two compositors from `compositor` commit the same transactions. One draws each frame for its
/// target's age, with whatever damage, shifts and resident chunks it plans. The other forgets its
/// caches before every frame, so it redraws the whole output.
pub fn incremental_equals_whole(compositor: MakeCompositor<'_>, target: MakeTarget<'_>) {
    let pair = Pair {
        compositor,
        target,
        caps: None,
    };
    play(
        (pair, false),
        (pair, true),
        INCREMENTAL,
        "an incremental frame and the whole redraw",
    );
}

/// One transcript of random edits drawn by two pairs gives the same images within `tolerance`.
///
/// Each pair draws its frames incrementally, with the damage its compositor plans. Effects use
/// only the blend modes both renderers draw.
pub fn pairs_agree(first: Pair<'_>, second: Pair<'_>, tolerance: Tolerance) {
    play(
        (first, false),
        (second, false),
        tolerance,
        "the first pair and the second",
    );
}

/// A list the compositor scrolls while its client changes it on every frame, drawn
/// incrementally, equals the same frames redrawn whole.
///
/// The client changes a shared paint, an opacity group and inline-coloured rows during wheel
/// glides, touchpad gestures, flings and bounces, acknowledges offsets in some of those commits,
/// and a thumb follows the offset beside the port.
///
/// Returns how many incremental frames shifted pixels, or zero when the compositor takes no
/// scroll input.
pub fn driven_motion_equals_whole(compositor: MakeCompositor<'_>, target: MakeTarget<'_>) -> u32 {
    let pair = Pair {
        compositor,
        target,
        caps: None,
    };
    let cadence = driven::Cadence {
        rows: (0, 4),
        record: 5,
        acknowledge: 4,
        jump: None,
    };
    driven::play(pair, driven::glides(), cadence)
}

/// A list the compositor scrolls while its client adopts or writes the newest offset in every
/// commit, drawn incrementally, equals the same frames redrawn whole.
///
/// Touchpad updates come before and after each commit, so the commits acknowledge while an update
/// waits for the next frame, and the frame shows an update that came after the commit. The
/// commits change rows as well, and every third writes an offset of its own in place of the
/// acknowledgement.
///
/// Returns how many incremental frames shifted pixels, or zero when the compositor takes no
/// scroll input.
pub fn adopted_motion_equals_whole(compositor: MakeCompositor<'_>, target: MakeTarget<'_>) -> u32 {
    let pair = Pair {
        compositor,
        target,
        caps: None,
    };
    let cadence = driven::Cadence {
        rows: (2, driven::ROWS as usize),
        record: 1,
        acknowledge: 1,
        jump: Some(3),
    };
    driven::play(pair, driven::drags(), cadence)
}
