//! Emboldened bitmaps: a glyph's outline filled and stroked.

use iceblink::{GlyphBitmap, GlyphKey, GlyphPlacement, PenPosition, RasterStyle};
use swash::scale::outline::Outline;
use swash::zeno::{Fill, Format, Join, Mask, Origin, Stroke, Style, Vector};

use crate::raster::bitmap::blank;
use crate::raster::render::phase;
use crate::raster::stripes::STRIPE_OFFSETS;

/// The coverage buffers an emboldened bitmap is rendered into, kept for the next one.
#[derive(Default)]
pub(super) struct Layers {
    fill: Vec<u8>,
    stroke: Vec<u8>,
}

/// Rasterises `outline` grown by the emboldening of `key` into `out`, at the key's phase, and
/// returns where the bitmap sits.
///
/// The outline is filled and stroked twice the emboldening wide with round joins, so every side
/// grows by the emboldening. The bitmap covers the box of the outline's points grown by the
/// emboldening on every side, as `PenPosition::bitmap_box` gives it, in the layout of the key's
/// style, which is `Mono` or `Subpixel`. An outline without points gives a zero-sized bitmap.
pub(super) fn rasterize(
    outline: &Outline,
    key: &GlyphKey,
    layers: &mut Layers,
    out: &mut GlyphBitmap,
) -> GlyphPlacement {
    let points = swash::zeno::bounds(outline.path(), Fill::NonZero, None);
    if points.is_empty() {
        blank(out);
        return GlyphPlacement::default();
    }

    let embolden = key.embolden();
    // Outlines grow upwards, and the box is measured downwards.
    let bounds = [
        points.min.x - embolden,
        -points.max.y - embolden,
        points.max.x + embolden,
        -points.min.y + embolden,
    ];
    let subpixel = key.style == RasterStyle::Subpixel;
    let phase = phase(key);
    let Some(rect) = PenPosition::of(phase).bitmap_box(0.0, bounds, subpixel) else {
        blank(out);
        return GlyphPlacement::default();
    };
    let [left, top] = [rect.origin.x, rect.origin.y];
    let [width, height] = [rect.size.width as u32, rect.size.height as u32];

    let format = if subpixel {
        Format::CustomSubpixel(STRIPE_OFFSETS)
    } else {
        Format::Alpha
    };
    let bottom = top + height as i32;
    let offset = Vector::new(phase - left as f32, bottom as f32);
    layers.render(outline, embolden, format, offset, [width, height]);

    out.width = width;
    out.height = height;
    out.bytes.clear();
    layers.merge_into(subpixel, &mut out.bytes);
    GlyphPlacement { left, top: -top }
}

impl Layers {
    /// Renders the fill of `outline` and its stroke `2 * embolden` wide into the two layers.
    fn render(
        &mut self,
        outline: &Outline,
        embolden: f32,
        format: Format,
        offset: Vector,
        [width, height]: [u32; 2],
    ) {
        let mut stroke = Stroke::new(2.0 * embolden);
        stroke.join(Join::Round);
        let styles: [Style<'_>; 2] = [Fill::NonZero.into(), stroke.into()];
        for (layer, style) in [&mut self.fill, &mut self.stroke].into_iter().zip(styles) {
            layer.clear();
            layer.resize(format.buffer_size(width, height), 0);
            Mask::new(outline.path())
                .style(style)
                .format(format)
                .origin(Origin::BottomLeft)
                .offset(offset)
                .size(width, height)
                .render_into(layer, None);
        }
    }

    /// Appends the larger coverage of the two layers to `bytes`, three bytes per pixel if
    /// `subpixel` and one otherwise.
    fn merge_into(&self, subpixel: bool, bytes: &mut Vec<u8>) {
        let covered = self
            .fill
            .iter()
            .zip(&self.stroke)
            .map(|(fill, stroke)| *fill.max(stroke));
        if subpixel {
            // The fourth byte of every pixel is unused.
            bytes.extend(
                covered
                    .enumerate()
                    .filter(|(index, _)| index % 4 != 3)
                    .map(|(_, coverage)| coverage),
            );
        } else {
            bytes.extend(covered);
        }
    }
}
