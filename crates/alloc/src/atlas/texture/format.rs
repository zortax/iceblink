//! Pixel formats of atlas textures.

/// The pixel format of an atlas texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TextureFormat {
    /// One byte of coverage per texel.
    R8Unorm,
    /// Four bytes per texel in red, green, blue, alpha order, with premultiplied alpha.
    ///
    /// Uploaded colour channels must already be multiplied by their alpha.
    Rgba8Unorm,
}

impl TextureFormat {
    /// Returns the size of one texel in bytes.
    pub const fn bytes_per_texel(self) -> u32 {
        match self {
            Self::R8Unorm => 1,
            Self::Rgba8Unorm => 4,
        }
    }

    /// Returns the size in bytes of a tightly packed `width` by `height` rectangle.
    pub const fn bytes_for(self, width: u32, height: u32) -> u64 {
        (self.bytes_per_texel() as u64) * (width as u64) * (height as u64)
    }
}
