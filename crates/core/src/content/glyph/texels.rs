//! Glyph bitmaps converted into the texels of their pages.

use iceblink::{GlyphBitmap, RasterStyle};
use iceblink_alloc::atlas::TextureKind;

/// Returns the pool a glyph of `style` is placed in.
pub(crate) const fn pool_of(style: RasterStyle) -> TextureKind {
    match style {
        RasterStyle::Mono => TextureKind::Mono,
        RasterStyle::Subpixel => TextureKind::Subpixel,
        RasterStyle::Color => TextureKind::Color,
    }
}

/// Returns the page texels of `bitmap`, rasterised in `style`.
///
/// Mono coverage is copied. Subpixel coverage gets a fourth byte per pixel. Colour pixels are
/// premultiplied.
pub(crate) fn texels(bitmap: &GlyphBitmap, style: RasterStyle) -> Vec<u8> {
    match style {
        RasterStyle::Mono => bitmap.bytes.clone(),
        RasterStyle::Subpixel => pad_to_four(&bitmap.bytes),
        RasterStyle::Color => premultiply(&bitmap.bytes),
    }
}

/// Widens three coverage bytes per pixel to four, with an opaque fourth byte.
fn pad_to_four(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() / 3 * 4);
    for pixel in bytes.as_chunks::<3>().0 {
        out.extend_from_slice(pixel);
        out.push(u8::MAX);
    }
    out
}

/// Multiplies each colour channel by its pixel's alpha, rounding to nearest.
pub(crate) fn premultiply(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for pixel in bytes.as_chunks::<4>().0 {
        let alpha = u32::from(pixel[3]);
        for channel in &pixel[..3] {
            out.push(((u32::from(*channel) * alpha + 127) / 255) as u8);
        }
        out.push(pixel[3]);
    }
    out
}
