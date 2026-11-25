//! Quads that replace every pixel of a rectangle.

use crate::geometry::DeviceRect;

/// A quad that writes an opaque colour to every pixel of a rectangle.
///
/// For a draw rectangle inside `rect`, a renderer may skip the clear, every batch before `batch`,
/// and the instances of `batch` before `at`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpaqueCover {
    /// The pixels the quad replaces.
    pub rect: DeviceRect,
    /// The index of the batch that draws the quad.
    pub batch: u32,
    /// The quad's position in the quads lane, counting instances.
    pub at: u32,
}
