//! The output seam: surfaces, image rotation, partial presentation and pacing.

use core::any::Any;

use crate::damage::DamageSet;
use crate::{
    Acquired, PresentCapabilities, PresentFeedback, PresentResult, STALE_DAMAGE, SkipReason,
};

/// Everything between a composed image and the display: surfaces, image rotation, partial
/// presentation and pacing.
///
/// A presenter works in one of the two modes its capabilities report:
///
/// - In kept-target mode, the renderer composes into a target it keeps between frames. It then
///   acquires an image and copies into it only the image's debt, the `present` damage of every
///   frame composed since the image was last written. A new image owes the whole target.
/// - In buffer-age mode, the renderer composes into the presented image, which is acquired
///   before the frame is built. `composed` does nothing and every debt is empty.
///
/// A presenter reports declined, timed-out and unacquired images with the matching
/// `SkipReason`, and a lost surface it made again with `Skipped(Outdated)`. Times are on the
/// frame clock.
pub trait Presenter {
    /// Returns what the presenter supports.
    fn capabilities(&self) -> PresentCapabilities;

    /// Configures the images for a new size and opacity.
    ///
    /// Returns the reason when the surface cannot be configured.
    fn configure(&mut self, size: [u32; 2], opaque: bool) -> Result<(), SkipReason>;

    /// Takes the next image to compose or copy into.
    fn acquire(&mut self) -> Acquired;

    /// Returns the image acquired and neither presented nor released, if any.
    fn acquired(&self) -> Option<u32>;

    /// Returns an acquired image to the presenter without presenting it.
    fn release(&mut self, image: u32);

    /// Adds the `present` damage of a frame composed into the kept target to every image's debt.
    ///
    /// Every composed frame adds its damage once, including frames that are then held, declined
    /// or time out.
    fn composed(&mut self, damage: &DamageSet);

    /// Returns what a copy into `image` must cover.
    fn debt(&self, image: u32) -> &DamageSet<STALE_DAMAGE>;

    /// Presents the acquired `image` and clears its debt.
    ///
    /// `damage` holds the pixels that changed since the image presented before it. A presenter
    /// with `partial_present` passes it, or a region that contains it, to the platform.
    fn present(&mut self, image: u32, damage: &DamageSet) -> PresentResult;

    /// Returns when the next frame should start building to be shown at the next refresh, if the
    /// platform paces frames.
    fn next_deadline(&self) -> Option<u64>;

    /// Moves the timings of images shown on the display into `out`.
    fn feedback(&mut self, out: &mut Vec<PresentFeedback>);

    /// Returns the display's refresh interval, when the platform tells the presenter.
    ///
    /// The default is `None`.
    fn refresh_ns(&self) -> Option<u64> {
        None
    }

    /// Returns how many presents the display holds at once when it is given one every refresh.
    ///
    /// An image presented just after a refresh is shown that many refreshes later at the
    /// earliest. The default is one.
    fn display_depth(&self) -> u32 {
        1
    }

    /// Returns how many images the presenter presented, which is the number of the newest
    /// present.
    ///
    /// Each present's feedback reports this number in `PresentFeedback::present`. The default is
    /// zero, for a presenter that numbers no presents.
    fn presents(&self) -> u64 {
        0
    }

    /// Returns the presenter as its concrete type, for backend-specific companions.
    ///
    /// A wrapper presenter forwards the call to what it wraps.
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        None
    }
}
