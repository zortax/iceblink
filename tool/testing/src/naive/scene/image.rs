//! Image pixels, held premultiplied.

use std::sync::Arc;

use iceblink::{ImageDesc, ImageFormat, ImageWrite};

/// An image resource, with its texels as premultiplied RGBA8.
#[derive(Clone, Debug)]
pub(crate) struct StoredImage {
    pub(crate) desc: ImageDesc,
    /// Four bytes per texel, row after row.
    pub(crate) texels: Arc<Vec<u8>>,
    /// A version that changes with every write and is unique across all images.
    pub(crate) version: u64,
}

impl StoredImage {
    /// Returns a new image of transparent texels.
    pub(crate) fn new(desc: ImageDesc, version: u64) -> Self {
        let texels = desc.width as usize * desc.height as usize * 4;
        Self {
            desc,
            texels: Arc::new(vec![0; texels]),
            version,
        }
    }

    /// Reports whether `write` lies inside the image and its bytes hold every row it reads.
    pub(crate) fn accepts(&self, write: &ImageWrite) -> bool {
        let rect = write.rect;
        let inside = rect.origin.x >= 0
            && rect.origin.y >= 0
            && rect.size.width >= 0
            && rect.size.height >= 0
            && i64::from(rect.origin.x) + i64::from(rect.size.width) <= i64::from(self.desc.width)
            && i64::from(rect.origin.y) + i64::from(rect.size.height)
                <= i64::from(self.desc.height);
        if !inside {
            return false;
        }

        if rect.size.height == 0 || rect.size.width == 0 {
            return true;
        }

        let row = rect.size.width as u64 * bytes_per_texel(self.desc.format);
        let stride = u64::from(write.stride);
        let end = write.offset as u64 + (rect.size.height as u64 - 1) * stride + row;
        stride >= row && end <= write.bytes.len() as u64
    }

    /// Replaces the texels of `write.rect`, premultiplying straight texels, and takes `version`.
    pub(crate) fn write(&mut self, write: &ImageWrite, version: u64) {
        let format = self.desc.format;
        let width = self.desc.width as usize;
        let texels = Arc::make_mut(&mut self.texels);
        let per = bytes_per_texel(format) as usize;

        for row in 0..write.rect.size.height as usize {
            let source = write.offset + row * write.stride as usize;
            let y = write.rect.origin.y as usize + row;
            for column in 0..write.rect.size.width as usize {
                let from = &write.bytes[source + column * per..source + (column + 1) * per];
                let x = write.rect.origin.x as usize + column;
                let at = (y * width + x) * 4;
                texels[at..at + 4].copy_from_slice(&premultiplied(format, from));
            }
        }

        self.version = version;
    }

    /// Returns the size of the stored texels in bytes.
    pub(crate) fn bytes(&self) -> u64 {
        self.texels.len() as u64
    }
}

fn bytes_per_texel(format: ImageFormat) -> u64 {
    if format == ImageFormat::A8 { 1 } else { 4 }
}

/// Converts one texel of `format` to premultiplied RGBA8.
fn premultiplied(format: ImageFormat, texel: &[u8]) -> [u8; 4] {
    if format == ImageFormat::A8 {
        // An alpha-only image is stored as premultiplied black at its alpha.
        return [0, 0, 0, texel[0]];
    }

    if format == ImageFormat::RGBA8 {
        let alpha = u32::from(texel[3]);
        let scale = |channel: u8| ((u32::from(channel) * alpha + 127) / 255) as u8;
        return [scale(texel[0]), scale(texel[1]), scale(texel[2]), texel[3]];
    }

    [texel[0], texel[1], texel[2], texel[3]]
}
