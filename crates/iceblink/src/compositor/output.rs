//! The configuration of an output.

/// The configuration of an output.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputConfig {
    /// The size, in device pixels.
    pub size: [u32; 2],
    /// Device pixels per logical pixel.
    pub scale: f32,
    /// Whether the final image is opaque.
    pub opaque: bool,
}
