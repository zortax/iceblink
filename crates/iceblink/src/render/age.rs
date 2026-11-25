//! What the image a frame is drawn into already holds.

/// What the image a frame is drawn into holds before the frame is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferAge {
    /// The image is a kept target that holds the previous frame of the output.
    Kept,
    /// The image holds the frame presented this many frames ago, at least 1.
    Known(u32),
    /// The image's contents are unknown.
    Unknown,
}
