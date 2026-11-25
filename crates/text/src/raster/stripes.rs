//! Subpixel bitmaps drawn from a glyph's outline.

use iceblink::{GlyphBitmap, GlyphPlacement};
use swash::scale::outline::Outline;
use swash::zeno::{Fill, Format, Mask, Origin, Vector};

use crate::raster::bitmap::blank;

/// How far the outline moves right for the red, green and blue coverage, in pixels.
///
/// The red stripe lies a third of a pixel left of the pixel's centre, so moving the outline right
/// by a third puts the red stripe's coverage at the centre. Blue is the mirror image.
pub(super) const STRIPE_OFFSETS: [f32; 3] = [1.0 / 3.0, 0.0, -1.0 / 3.0];

/// Rasterises `outline`, moved right by `phase` pixels, into `out` as three coverage bytes per
/// pixel in stripe order, and returns where the bitmap sits.
///
/// The bitmap covers the outline at every stripe's offset, so each stripe keeps all of its
/// coverage. An outline without area gives a zero-sized bitmap. `scratch` holds the four bytes
/// per pixel the mask is rendered into.
pub(super) fn rasterize(
    outline: &Outline,
    phase: f32,
    scratch: &mut Vec<u8>,
    out: &mut GlyphBitmap,
) -> GlyphPlacement {
    let bounds = swash::zeno::bounds(outline.path(), Fill::NonZero, None);
    if bounds.is_empty() {
        blank(out);
        return GlyphPlacement::default();
    }

    // The red offset reaches furthest right and the blue offset furthest left.
    let left = (bounds.min.x + phase + STRIPE_OFFSETS[2]).floor();
    let right = (bounds.max.x + phase + STRIPE_OFFSETS[0]).ceil();
    let bottom = bounds.min.y.floor();
    let top = bounds.max.y.ceil();
    let width = (right - left) as u32;
    let height = (top - bottom) as u32;

    scratch.clear();
    scratch.resize(width as usize * height as usize * 4, 0);
    Mask::new(outline.path())
        .format(Format::CustomSubpixel(STRIPE_OFFSETS))
        .origin(Origin::BottomLeft)
        .offset(Vector::new(phase - left, -bottom))
        .size(width, height)
        .render_into(scratch, None);

    out.width = width;
    out.height = height;
    out.bytes.clear();
    out.bytes.reserve(scratch.len() / 4 * 3);
    for pixel in scratch.as_chunks::<4>().0 {
        out.bytes.extend_from_slice(&pixel[..3]);
    }
    GlyphPlacement {
        left: left as i32,
        top: top as i32,
    }
}
