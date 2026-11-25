//! The pixels of every image, kept as client state.

use core::fmt;
use std::sync::Arc;

use iceblink::{ImageDesc, ImageFormat, ImageWrite};
use rustc_hash::FxHashMap;

use crate::content::image::texels::convert_rows;

/// The key of an image in an [`ImageStore`].
///
/// A key that is created again refers to a new image, which never shares a tile with the one
/// before.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImageKey(pub u64);

/// Why an image write was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImageError {
    /// No image has the key.
    UnknownImage,
    /// The written rectangle extends outside the image.
    OutOfBounds,
    /// The bytes end before the last texel of the rectangle, or rows overlap.
    ShortBytes,
    /// The image's format is not a listed format.
    InvalidFormat,
}

impl fmt::Display for ImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnknownImage => "no image has the key",
            Self::OutOfBounds => "the written rectangle reaches outside the image",
            Self::ShortBytes => "the bytes do not hold every texel of the rectangle",
            Self::InvalidFormat => "the image format is not a listed format",
        })
    }
}

impl std::error::Error for ImageError {}

/// One image's description and pixels.
#[derive(Debug)]
pub(crate) struct StoredImage {
    pub(crate) desc: ImageDesc,
    /// A number no other image has had, used in the keys of the image's tiles.
    pub(crate) serial: u64,
    /// Premultiplied, gamma-encoded sRGB, four bytes per texel, rows tightly packed.
    pub(crate) texels: Arc<Vec<u8>>,
}

/// The pixels of every image, which tiles are placed from.
///
/// Pixels are stored premultiplied, four bytes per texel, whatever the image's format. They are
/// client state, so eviction and forgetting the caches never drop them. One store can serve
/// several [`ContentCache`](crate::content::ContentCache)s.
#[derive(Debug, Default)]
pub struct ImageStore {
    images: FxHashMap<ImageKey, StoredImage>,
    next_serial: u64,
    bytes: u64,
}

impl ImageStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates the image of `key` with transparent texels, replacing any image the key had.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::InvalidFormat`] for a format that is not a listed one.
    pub fn create(&mut self, key: ImageKey, desc: ImageDesc) -> Result<(), ImageError> {
        if !desc.format.is_valid() {
            return Err(ImageError::InvalidFormat);
        }

        self.remove(key);
        let length = desc.width as usize * desc.height as usize * 4;
        self.next_serial += 1;
        self.bytes += length as u64;
        self.images.insert(
            key,
            StoredImage {
                desc,
                serial: self.next_serial,
                texels: Arc::new(vec![0; length]),
            },
        );
        Ok(())
    }

    /// Replaces the texels of `write.rect` in the image of `key`.
    ///
    /// Straight texels are premultiplied and alpha-only texels become black at their alpha.
    ///
    /// # Errors
    ///
    /// - [`ImageError::UnknownImage`] if no image has the key.
    /// - [`ImageError::OutOfBounds`] if the rectangle is empty or extends outside the image.
    /// - [`ImageError::ShortBytes`] if a row is shorter than the rectangle or the bytes end before
    ///   its last texel.
    pub fn write(&mut self, key: ImageKey, write: &ImageWrite) -> Result<(), ImageError> {
        let image = self.images.get_mut(&key).ok_or(ImageError::UnknownImage)?;
        let rect = written_rect(image.desc, write)?;
        check_bytes(image.desc.format, rect, write)?;
        convert_rows(
            image.desc.format,
            &write.bytes[write.offset..],
            write.stride as usize,
            rect,
            image.desc.width as usize,
            Arc::make_mut(&mut image.texels).as_mut_slice(),
        );
        Ok(())
    }

    /// Returns every image, in no particular order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &StoredImage> {
        self.images.values()
    }

    /// Removes the image of `key`, and returns whether there was one.
    pub fn remove(&mut self, key: ImageKey) -> bool {
        let Some(image) = self.images.remove(&key) else {
            return false;
        };

        self.bytes -= image.texels.len() as u64;
        true
    }

    /// Returns the description of the image of `key`.
    pub fn desc(&self, key: ImageKey) -> Option<ImageDesc> {
        self.images.get(&key).map(|image| image.desc)
    }

    /// Returns the premultiplied texel at `x`, `y` of the image of `key`.
    pub fn texel(&self, key: ImageKey, x: u32, y: u32) -> Option<[u8; 4]> {
        let image = self.images.get(&key)?;
        if x >= image.desc.width || y >= image.desc.height {
            return None;
        }

        let at = (y as usize * image.desc.width as usize + x as usize) * 4;
        image.texels.get(at..at + 4)?.try_into().ok()
    }

    /// Returns the bytes of every image's pixels.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Returns the image of `key`.
    pub(crate) fn get(&self, key: ImageKey) -> Option<&StoredImage> {
        self.images.get(&key)
    }
}

/// Returns the written rectangle as `[x, y, width, height]` texels, if it lies inside the image.
pub(crate) fn written_rect(desc: ImageDesc, write: &ImageWrite) -> Result<[usize; 4], ImageError> {
    let origin = write.rect.origin;
    let size = write.rect.size;
    if origin.x < 0 || origin.y < 0 || size.width <= 0 || size.height <= 0 {
        return Err(ImageError::OutOfBounds);
    }

    let [x, y, width, height] = [origin.x, origin.y, size.width, size.height].map(|v| v as usize);
    if x + width > desc.width as usize || y + height > desc.height as usize {
        return Err(ImageError::OutOfBounds);
    }

    Ok([x, y, width, height])
}

/// Checks that the bytes of `write` hold every texel of `rect` in `format`.
fn check_bytes(
    format: ImageFormat,
    rect: [usize; 4],
    write: &ImageWrite,
) -> Result<(), ImageError> {
    let row = rect[2] * bytes_per_texel(format);
    let stride = write.stride as usize;
    if stride < row {
        return Err(ImageError::ShortBytes);
    }

    let needed = (rect[3] - 1)
        .checked_mul(stride)
        .and_then(|rows| rows.checked_add(row))
        .and_then(|span| span.checked_add(write.offset));
    match needed {
        Some(end) if end <= write.bytes.len() => Ok(()),
        _ => Err(ImageError::ShortBytes),
    }
}

/// Returns the bytes one texel of `format` takes in a write.
pub(crate) const fn bytes_per_texel(format: ImageFormat) -> usize {
    if format.0 == ImageFormat::A8.0 { 1 } else { 4 }
}
