//! Glyph outlines: placement against the bitmap, size, slant and the cache.

mod common;

use iceblink::GlyphRaster;
use iceblink::kurbo::{BezPath, Shape};
use iceblink::{GlyphBitmap, OutlineKey, RasterStyle};
use iceblink_text::SwashRaster;

use crate::common::{LATIN, font, glyph, key, outline_key};

/// Returns the outline of `key` of the Latin face.
fn outline(raster: &SwashRaster, key: &OutlineKey) -> BezPath {
    let mut path = BezPath::new();
    assert!(
        raster.outline(&font(LATIN), key, &[], &mut path),
        "{key:?} has an outline"
    );
    path
}

#[test]
fn an_outline_covers_the_ink_its_bitmap_covers() {
    let latin = font(LATIN);
    let h = glyph(&latin, 'H');
    let raster = SwashRaster::new();
    let mut bitmap = GlyphBitmap::default();
    let placement = raster
        .rasterize(&latin, &key(h, 64.0, RasterStyle::Mono), &[], &mut bitmap)
        .expect("the glyph exists");
    let bounds = outline(&raster, &outline_key(h, 64.0)).bounding_box();

    // The placement is measured up from the baseline, and outlines grow down.
    assert!(
        (bounds.y0 + f64::from(placement.top)).abs() <= 1.5,
        "{bounds:?} {placement:?}"
    );
    assert!(
        (bounds.x0 - f64::from(placement.left)).abs() <= 1.5,
        "{bounds:?} {placement:?}"
    );
    assert!((bounds.width() - f64::from(bitmap.width)).abs() <= 2.0);
    assert!((bounds.height() - f64::from(bitmap.height)).abs() <= 2.0);
}

#[test]
fn an_outline_is_drawn_at_the_key_size() {
    let h = glyph(&font(LATIN), 'H');
    let raster = SwashRaster::new();
    let small = outline(&raster, &outline_key(h, 32.0)).bounding_box();
    let large = outline(&raster, &outline_key(h, 128.0)).bounding_box();
    let ratio = large.height() / small.height();
    assert!(
        (ratio - 4.0).abs() < 0.05,
        "four times the size is {ratio} times the height"
    );
}

#[test]
fn a_slant_shears_the_outline_about_the_baseline() {
    let l = glyph(&font(LATIN), 'l');
    let raster = SwashRaster::new();
    let upright = outline(&raster, &outline_key(l, 96.0)).bounding_box();
    let leaning = outline(
        &raster,
        &OutlineKey {
            skew_bits: 14.0f32.to_bits(),
            ..outline_key(l, 96.0)
        },
    )
    .bounding_box();
    assert!(
        leaning.x1 > upright.x1 + 5.0,
        "{leaning:?} against {upright:?}"
    );
    assert!(
        (leaning.y1 - upright.y1).abs() < 0.001,
        "the baseline does not move"
    );
}

#[test]
fn a_cached_outline_equals_the_first_extraction() {
    let g = glyph(&font(LATIN), 'g');
    let raster = SwashRaster::new();
    let first = outline(&raster, &outline_key(g, 96.0));
    let mut second = BezPath::new();
    second.move_to((1.0, 1.0));
    assert!(raster.outline(&font(LATIN), &outline_key(g, 96.0), &[], &mut second));
    assert_eq!(first, second);
}

#[test]
fn a_blank_glyph_has_an_empty_outline_and_a_missing_one_has_none() {
    let latin = font(LATIN);
    let raster = SwashRaster::new();
    let mut path = BezPath::new();
    assert!(raster.outline(
        &latin,
        &outline_key(glyph(&latin, ' '), 32.0),
        &[],
        &mut path
    ));
    assert!(path.elements().is_empty());
    assert!(!raster.outline(&latin, &outline_key(70_000, 32.0), &[], &mut path));
}
