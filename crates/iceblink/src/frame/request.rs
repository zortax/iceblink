//! What a frame is built for, and what building it produced.

use crate::frame::FrameClock;
use crate::render::{BufferAge, RenderCapabilities, SubpixelOrder};

/// What a frame is built for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameRequest<'a> {
    /// The capabilities of the renderer that draws the frame, read from it before the build.
    pub caps: &'a RenderCapabilities,
    /// What the image the frame is drawn into holds.
    pub age: BufferAge,
    /// When the frame is built and presented.
    pub clock: FrameClock,
    /// The display's stripe order, when subpixel sprites may be drawn.
    pub subpixel: Option<SubpixelOrder>,
}

/// What building a frame produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Built {
    /// The frame holds work to draw.
    Frame,
    /// Nothing changed, and the frame is untouched and must not be drawn.
    Idle {
        /// When a scheduled change, such as the end of an animation's delay, needs a frame.
        wake_ns: Option<u64>,
    },
}
