//! Images.

use std::sync::Arc;

use crate::bits::enumeration;
use crate::geometry::DeviceRect;

/// The size and texel format of an image.
///
/// A new image holds transparent texels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageDesc {
    /// The width, in texels.
    pub width: u32,
    /// The height, in texels.
    pub height: u32,
    /// The layout of a texel.
    pub format: ImageFormat,
}

enumeration! {
    /// The layout of an image's texels. Colour is gamma-encoded sRGB.
    pub struct ImageFormat(u8) {
        /// Four bytes per texel with premultiplied alpha.
        RGBA8_PREMULTIPLIED = 0,
        /// Four bytes per texel with straight alpha.
        RGBA8 = 1,
        /// One byte per texel: alpha only.
        A8 = 2,
    }
}

/// New texels for a rectangle of an image.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageWrite {
    /// The texels written, in texels of the image.
    pub rect: DeviceRect,
    /// The bytes holding the texels, in the image's format.
    pub bytes: Arc<[u8]>,
    /// Where the first texel starts in `bytes`.
    pub offset: usize,
    /// The distance in bytes from one row to the next.
    pub stride: u32,
}
