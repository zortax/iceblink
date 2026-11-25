//! Backdrops in a frame.

use std::sync::Arc;

use crate::frame::FrameFilter;
use crate::item::Backdrop;

/// A backdrop, with the device rectangles it writes and reads.
///
/// The backdrop reads `source` from the target open at its batch, applies `filters`, and paints
/// the result over `bounds`.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameBackdrop {
    /// The backdrop, with frame slots.
    pub backdrop: Backdrop,
    /// The device rectangle written, as `[x, y, width, height]`.
    pub bounds: [f32; 4],
    /// The device rectangle read, as `[x, y, width, height]`.
    pub source: [f32; 4],
    /// The filter chain.
    pub filters: Arc<[FrameFilter]>,
}
