//! The boxes of unhinted bitmaps, which follow from the glyphs' outlines and emboldening.

mod common;

use iceblink::GlyphRaster;
use iceblink::geom::{Point, Rect, Size};
use iceblink::kurbo::BezPath;
use iceblink::{GlyphBitmap, PHASES, PenPosition, RasterStyle};
use iceblink_text::SwashRaster;
use skrifa::raw::TableProvider;

use crate::common::{LATIN, font, key, outline_key};

#[test]
fn an_unhinted_bitmap_covers_the_box_of_its_outline_points_grown_by_the_emboldening() {
    let latin = font(LATIN);
    let face = skrifa::FontRef::new(latin.bytes()).expect("the test font parses");
    let count = u32::from(face.maxp().expect("the font has a maxp table").num_glyphs());
    let raster = SwashRaster::new();
    let mut bitmap = GlyphBitmap::default();
    let mut path = BezPath::new();
    let mut compared = 0;
    for glyph in (1..count).step_by(23) {
        for size in [9.5f32, 13.5, 24.0] {
            for skew in [0.0f32, 12.0] {
                let mut outline = outline_key(glyph, size);
                outline.skew_bits = skew.to_bits();
                assert!(raster.outline(&latin, &outline, &[], &mut path));
                let points = path.control_box();
                let bounds = [
                    points.x0 as f32,
                    points.y0 as f32,
                    points.x1 as f32,
                    points.y1 as f32,
                ];
                for (style, phase, embolden) in styles() {
                    let mut key = key(glyph, size, style);
                    key.phase = phase;
                    key.skew_bits = skew.to_bits();
                    key.embolden_bits = embolden.to_bits();
                    let bounds = [
                        bounds[0] - embolden,
                        bounds[1] - embolden,
                        bounds[2] + embolden,
                        bounds[3] + embolden,
                    ];
                    let placement = raster
                        .rasterize(&latin, &key, &[], &mut bitmap)
                        .expect("the font has every glyph below its count");
                    let pen = PenPosition::of(f32::from(phase) / f32::from(PHASES));
                    let expected = if path.elements().is_empty() {
                        None
                    } else {
                        pen.bitmap_box(0.0, bounds, style == RasterStyle::Subpixel)
                    };
                    let drawn = (!bitmap.is_empty()).then(|| {
                        Rect::new(
                            Point::new(placement.left, -placement.top),
                            Size::new(bitmap.width as i32, bitmap.height as i32),
                        )
                    });

                    assert_eq!(
                        drawn, expected,
                        "glyph {glyph} at {size} px, skew {skew}, {style:?}, phase {phase}, \
                             emboldening {embolden}"
                    );
                    compared += 1;
                }
            }
        }
    }
    assert!(compared > 1_000, "the sweep compared {compared} bitmaps");
}

/// Returns every style, phase and emboldening the sweep rasterises.
fn styles() -> impl Iterator<Item = (RasterStyle, u8, f32)> {
    [RasterStyle::Mono, RasterStyle::Subpixel]
        .into_iter()
        .flat_map(|style| (0..PHASES).map(move |phase| (style, phase)))
        .flat_map(|(style, phase)| {
            [0.0, 0.6, 1.25]
                .into_iter()
                .map(move |embolden| (style, phase, embolden))
        })
}
