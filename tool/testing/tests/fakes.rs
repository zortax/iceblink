//! The fake glyph rasteriser, font registry and renderer.

mod common;

use iceblink::damage::DamageSet;
use iceblink::kurbo::{BezPath, Shape};
use iceblink::{
    BufferAge, FontId, FrameOutcome, GlyphBitmap, GlyphKey, OutlineKey, RasterStyle, RenderTarget,
    SkipReason,
};
use iceblink::{FontRegistry, GlyphRaster, Renderer};
use iceblink_testing::fakes::{FakeFontRegistry, FakeGlyphRaster, NullRenderer, fake_font};

use crate::common::frames;

/// The key of `glyph` at 16 pixels, phase `phase`, in `style`.
fn key(glyph: u32, phase: u8, style: RasterStyle) -> GlyphKey {
    GlyphKey {
        font: FontId(0),
        glyph,
        size_bits: 16.0f32.to_bits(),
        instance: 0,
        phase,
        style,
        hint: false,
        palette: 0,
        embolden_bits: 0,
        skew_bits: 0,
    }
}

/// A square opaque target with the side of the test frames.
fn target() -> RenderTarget {
    RenderTarget {
        size: [frames::SIDE, frames::SIDE],
        scale: 1.0,
        opaque: true,
    }
}

#[test]
fn a_fake_glyph_is_a_box_on_the_baseline_in_every_style() {
    let raster = FakeGlyphRaster::new();
    let font = fake_font(0);
    let mut bitmap = GlyphBitmap::default();
    for style in [RasterStyle::Mono, RasterStyle::Subpixel, RasterStyle::Color] {
        let placement = raster
            .rasterize(&font, &key(65, 0, style), &[], &mut bitmap)
            .expect("every glyph below MISSING exists");
        assert_eq!((bitmap.width, bitmap.height), (8, 12));
        assert_eq!((placement.left, placement.top), (0, 12));
        assert!(bitmap.is_well_formed(style), "{style:?}");
    }
    assert_eq!(raster.rasterised(), 3);
}

#[test]
fn a_fake_glyph_at_a_phase_gains_partial_columns() {
    let raster = FakeGlyphRaster::new();
    let mut bitmap = GlyphBitmap::default();
    raster.rasterize(
        &fake_font(0),
        &key(65, 2, RasterStyle::Mono),
        &[],
        &mut bitmap,
    );
    assert_eq!(bitmap.width, 9);
    assert_eq!(&bitmap.bytes[..2], &[127, 255]);
    assert_eq!(bitmap.bytes[8], 127);
}

#[test]
fn a_blank_glyph_is_empty_and_a_missing_one_is_none() {
    let raster = FakeGlyphRaster::new();
    let mut bitmap = GlyphBitmap::default();
    let font = fake_font(0);
    let blank = raster.rasterize(
        &font,
        &key(FakeGlyphRaster::BLANK, 0, RasterStyle::Mono),
        &[],
        &mut bitmap,
    );
    assert!(blank.is_some() && bitmap.is_empty());
    let missing = raster.rasterize(
        &font,
        &key(FakeGlyphRaster::MISSING, 0, RasterStyle::Mono),
        &[],
        &mut bitmap,
    );
    assert!(missing.is_none());
}

#[test]
fn a_fake_outline_is_the_box() {
    let raster = FakeGlyphRaster::new();
    let mut path = BezPath::new();
    let outline = OutlineKey {
        font: FontId(0),
        glyph: 65,
        size_bits: 16.0f32.to_bits(),
        instance: 0,
        skew_bits: 0,
    };
    assert!(raster.outline(&fake_font(0), &outline, &[], &mut path));
    let bounds = path.bounding_box();
    assert_eq!(
        (bounds.x0, bounds.y0, bounds.x1, bounds.y1),
        (0.0, -12.0, 8.0, 0.0)
    );
}

#[test]
fn a_registry_returns_one_id_per_font() {
    let registry = FakeFontRegistry::new();
    let first = registry.register(fake_font(1));
    let second = registry.register(fake_font(2));
    assert_ne!(first, second);
    assert_eq!(registry.register(fake_font(1)), first);
    assert_eq!(registry.generation(), 2);
    assert_eq!(registry.get(second), Some(fake_font(2)));
    assert_eq!(registry.get(FontId(9)), None);
}

#[test]
fn a_null_renderer_keeps_its_target_once_a_frame_retires_damage() {
    let mut renderer = NullRenderer::new();
    assert_eq!(
        renderer.draw(&frames::first()),
        FrameOutcome::Skipped(SkipReason::Unconfigured)
    );
    assert_eq!(renderer.target_age(), BufferAge::Unknown);

    renderer.configure(target());
    let outcome = renderer.draw(&frames::first());
    assert_eq!(outcome.stats().map(|stats| stats.damage_px), Some(64 * 64));
    assert_eq!(renderer.target_age(), BufferAge::Kept);

    renderer.push_outcome(FrameOutcome::Recovered);
    assert_eq!(renderer.draw(&frames::first()), FrameOutcome::Recovered);
    assert_eq!(renderer.target_age(), BufferAge::Unknown);
    assert_eq!(renderer.draws().len(), 3);
}

#[test]
fn a_null_renderer_presents_a_held_frame_once() {
    let mut renderer = NullRenderer::new();
    renderer.configure(target());
    renderer.push_outcome(FrameOutcome::Skipped(SkipReason::Held));
    renderer.draw(&frames::first());
    assert!(renderer.present_held().stats().is_some());
    assert_eq!(
        renderer.present_held(),
        FrameOutcome::Skipped(SkipReason::Undamaged)
    );
}

#[test]
fn a_validating_null_renderer_collects_violations() {
    let mut renderer = NullRenderer::new().validating();
    renderer.configure(target());
    renderer.draw(&frames::with_quad());
    assert!(
        renderer.violations().is_empty(),
        "{:?}",
        renderer.violations()
    );

    let mut frame = frames::first();
    let mut damage = DamageSet::new();
    damage.absorb(frames::rect(60, 60, 8, 8));
    frame.damage.draw = damage;
    renderer.draw(&frame);
    assert_eq!(renderer.violations().len(), 1);
}
