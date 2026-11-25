//! What presenters support and report.

use crate::damage::MAX_DAMAGE;
use crate::render::{BufferAge, SkipReason};

/// What a presenter supports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentCapabilities {
    /// How composed frames are written into the presented images.
    pub mode: PresentMode,
    /// Whether the platform takes the changed rectangles with each present.
    pub partial_present: bool,
    /// Whether the platform reports when frames are shown on the display.
    pub feedback: bool,
}

/// How composed frames are written into the presented images.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentMode {
    /// The renderer composes into a target it keeps, then acquires an image and copies into it
    /// what the image still owes.
    KeptTarget,
    /// The renderer composes into the presented image, which is acquired before the frame is
    /// built.
    BufferAge,
}

/// How many rectangles an image's debt is tracked as.
pub const STALE_DAMAGE: usize = 2 * MAX_DAMAGE;

/// The answer to an acquire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acquired {
    /// An image to compose or copy into, and what it holds.
    Image {
        /// The image's index in the presenter.
        image: u32,
        /// What the image holds.
        age: BufferAge,
    },
    /// No image was acquired.
    Skipped(SkipReason),
}

/// The result of a present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentResult {
    /// The image was handed to the platform.
    Presented,
    /// The platform was not ready, and the image waits for it.
    Held,
    /// The image was not presented.
    Skipped(SkipReason),
}

/// When a presented image was shown on the display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentFeedback {
    /// The image's index in the presenter.
    pub image: u32,
    /// The number of the present, as `Presenter::presents` counted it, or zero when the presenter
    /// numbers no presents.
    pub present: u64,
    /// When the image was shown on the display, in nanoseconds on the frame clock.
    pub presented_ns: u64,
    /// The display's refresh interval at that time, or zero when the presenter does not know it.
    pub refresh_ns: u64,
}
