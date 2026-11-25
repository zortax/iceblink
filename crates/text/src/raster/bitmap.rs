//! Repacking a swash image into the bitmap layout of a raster style.

use iceblink::{GlyphBitmap, GlyphPlacement, RasterStyle};
use swash::scale::Source;
use swash::scale::image::{Content, Image};

/// Moves `image` into `out` in the layout of `style`, and returns where the bitmap sits.
///
/// The source the image came from decides its content, which may differ from `style`. Each
/// content is converted as follows:
///
/// - A mask is copied for mono, repeated in all three channels for subpixel, and white with the
///   coverage as alpha for colour.
/// - A subpixel mask keeps its first three bytes of every four for subpixel. For mono and colour,
///   its green coverage stands for the pixel.
/// - A colour image is straight colour for colour, and its alpha is the coverage for mono and
///   subpixel.
///
/// The image keeps an allocation for the next render.
pub(super) fn repack(
    image: &mut Image,
    style: RasterStyle,
    out: &mut GlyphBitmap,
) -> GlyphPlacement {
    let placement = GlyphPlacement {
        left: image.placement.left,
        top: image.placement.top,
    };
    out.width = image.placement.width;
    out.height = image.placement.height;
    out.bytes.clear();

    match (image.content, style) {
        (Content::Mask, RasterStyle::Mono) => std::mem::swap(&mut out.bytes, &mut image.data),
        (Content::Color, RasterStyle::Color) => {
            // Colour layers are composited premultiplied, and colour strikes decode straight.
            if matches!(image.source, Source::ColorOutline(_)) {
                unpremultiply(&mut image.data);
            }
            std::mem::swap(&mut out.bytes, &mut image.data);
        }
        (Content::SubpixelMask, RasterStyle::Subpixel) => {
            // The fourth byte is unused, and dropping it saves a quarter of every upload.
            out.bytes.reserve(image.data.len() / 4 * 3);
            for pixel in image.data.as_chunks::<4>().0 {
                out.bytes.extend_from_slice(&pixel[..3]);
            }
        }
        (Content::Mask, RasterStyle::Subpixel) => {
            for coverage in &image.data {
                out.bytes.extend_from_slice(&[*coverage; 3]);
            }
        }
        (Content::Mask, RasterStyle::Color) => {
            for coverage in &image.data {
                out.bytes
                    .extend_from_slice(&[u8::MAX, u8::MAX, u8::MAX, *coverage]);
            }
        }
        (Content::Color, RasterStyle::Mono) => {
            out.bytes
                .extend(image.data.as_chunks::<4>().0.iter().map(|pixel| pixel[3]));
        }
        (Content::Color, RasterStyle::Subpixel) => {
            for pixel in image.data.as_chunks::<4>().0 {
                out.bytes.extend_from_slice(&[pixel[3]; 3]);
            }
        }
        (Content::SubpixelMask, RasterStyle::Mono) => {
            out.bytes
                .extend(image.data.as_chunks::<4>().0.iter().map(|pixel| pixel[1]));
        }
        (Content::SubpixelMask, RasterStyle::Color) => {
            for pixel in image.data.as_chunks::<4>().0 {
                out.bytes
                    .extend_from_slice(&[u8::MAX, u8::MAX, u8::MAX, pixel[1]]);
            }
        }
    }
    placement
}

/// Empties `out` into a zero-sized bitmap.
pub(super) fn blank(out: &mut GlyphBitmap) {
    out.width = 0;
    out.height = 0;
    out.bytes.clear();
}

/// Divides the colour of every partly covered RGBA pixel by its alpha.
fn unpremultiply(pixels: &mut [u8]) {
    for pixel in pixels.as_chunks_mut::<4>().0 {
        let alpha = u32::from(pixel[3]);
        if alpha == 0 || alpha == 255 {
            continue;
        }
        for channel in &mut pixel[..3] {
            let straight = (u32::from(*channel) * 255 + alpha / 2) / alpha;
            *channel = straight.min(255) as u8;
        }
    }
}
