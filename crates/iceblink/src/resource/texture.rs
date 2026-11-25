//! External textures.

/// The size and alpha of an external texture slot.
///
/// The content of the slot comes from outside the compositor, such as a video decoder or another
/// process, and is bound to the slot through the renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureDesc {
    /// The width, in texels.
    pub width: u32,
    /// The height, in texels.
    pub height: u32,
    /// Whether the texels have premultiplied alpha.
    pub premultiplied: bool,
}
