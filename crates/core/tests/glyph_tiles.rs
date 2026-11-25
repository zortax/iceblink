//! Glyph tiles: reuse, phase quantisation, placement and blank glyphs.

mod common;

use common::content::{
    BLANK, FakeRaster, LEFT, MISSING, TOP, cache_with, glyph, input, lower, moved, run, solid,
    sprite_bounds,
};
use iceblink::{GlyphFlags, Matrix4, PenPosition};
use iceblink_core::content::{Lowered, keeps_pixels};

#[test]
fn a_glyph_seen_again_reuses_its_tile_without_rasterising() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let run = run(16.0, solid(3), GlyphFlags::default());
    let glyphs = [glyph(7, 10.0, 20.0), glyph(7, 30.0, 20.0)];

    let first = lower(&mut cache, &input(&run, &glyphs), &moved(0.0, 0.0));
    let second = lower(&mut cache, &input(&run, &glyphs), &moved(0.0, 0.0));

    assert_eq!(raster.count(), 1, "one key, one raster");
    assert_eq!(first.tiles[0], first.tiles[1]);
    assert_eq!(first.tiles, second.tiles);
    let tile = |lowering: &iceblink_core::content::Lowering, at: usize| match &lowering.items[at] {
        Lowered::Mono(sprite) => sprite.tile,
        other => panic!("expected a mono sprite, got {other:?}"),
    };
    assert_eq!(tile(&first, 0), tile(&first, 1));
    assert_eq!(tile(&first, 0), tile(&second, 0));
    assert_eq!(cache.stats().glyphs_rasterised, 1);
    assert_eq!(cache.stats().glyphs_placed, 4);
}

#[test]
fn positions_are_quantised_to_quarter_pixels_and_split_into_pen_and_phase() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let run = run(16.0, solid(0), GlyphFlags::default());
    let xs = [10.0, 10.2, 10.5, 10.74, 10.9];
    let glyphs: Vec<_> = xs.iter().map(|&x| glyph(5, x, 20.4)).collect();

    let lowering = lower(&mut cache, &input(&run, &glyphs), &moved(0.0, 0.0));

    let phases: Vec<u8> = raster.keys().iter().map(|key| key.phase).collect();
    assert_eq!(
        phases,
        [0, 1, 2, 3],
        "10.9 rounds up to the next pixel at phase zero"
    );
    let bounds = sprite_bounds(&lowering);
    for (x, bounds) in xs.iter().zip(&bounds) {
        let pen = PenPosition::of(*x);
        assert_eq!(bounds[0], pen.pen() + LEFT as f32);
        assert_eq!(
            bounds[1],
            20.0 - TOP as f32,
            "the baseline rounds to a whole pixel"
        );
        assert_eq!(bounds[0].fract(), 0.0);
    }
    assert_eq!(bounds[4][0], 11.0 + LEFT as f32);
}

#[test]
fn the_phase_comes_from_the_device_position_of_the_glyph() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let run = run(16.0, solid(0), GlyphFlags::default());
    let glyphs = [glyph(5, 10.0, 0.0)];

    let lowering = lower(&mut cache, &input(&run, &glyphs), &moved(0.5, 3.0));

    assert_eq!(raster.keys()[0].phase, 2);
    let bounds = sprite_bounds(&lowering)[0];
    assert_eq!(
        bounds[0] + 0.5,
        10.0 + LEFT as f32,
        "the sprite lands on a whole device pixel"
    );
    assert_eq!(bounds[1] + 3.0, 3.0 - TOP as f32);
}

#[test]
fn a_uniform_scale_rasterises_at_the_device_size_on_whole_device_pixels() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let run = run(12.0, solid(0), GlyphFlags::default());
    let glyphs = [glyph(5, 3.1, 8.0)];
    let world = Matrix4::scale(2.0, 2.0, 1.0);

    let lowering = lower(
        &mut cache,
        &input(&run, &glyphs),
        &iceblink_core::content::Target::new(world),
    );

    let key = raster.keys()[0];
    assert_eq!(key.size(), 24.0);
    assert_eq!(key.phase, PenPosition::of(6.2).phase());
    let bounds = sprite_bounds(&lowering)[0];
    assert_eq!((bounds[0] * 2.0).fract(), 0.0);
    assert_eq!((bounds[1] * 2.0).fract(), 0.0);
    assert_eq!(bounds[2] * 2.0, FakeRaster::width_of(&key) as f32);
}

#[test]
fn the_chunk_offset_moves_glyphs_and_their_paint_origin() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let run = run(16.0, solid(0), GlyphFlags::default());
    let glyphs = [glyph(5, 10.0, 20.0)];
    let mut target = moved(0.0, 0.0);
    target.offset = [4.25, -2.0];

    let lowering = lower(&mut cache, &input(&run, &glyphs), &target);

    assert_eq!(raster.keys()[0].phase, 1);
    match &lowering.items[0] {
        Lowered::Mono(sprite) => {
            assert_eq!(sprite.bounds[0], 14.0 + LEFT as f32);
            assert_eq!(sprite.bounds[1], 18.0 - TOP as f32);
            assert_eq!(sprite.paint_origin, [4.25, -2.0]);
        }
        other => panic!("expected a mono sprite, got {other:?}"),
    }
}

#[test]
fn blank_glyphs_are_remembered_and_missing_glyphs_draw_nothing() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let run = run(16.0, solid(0), GlyphFlags::default());
    let glyphs = [glyph(BLANK, 0.0, 0.0), glyph(MISSING, 4.0, 0.0)];

    let first = lower(&mut cache, &input(&run, &glyphs), &moved(0.0, 0.0));
    let second = lower(&mut cache, &input(&run, &glyphs), &moved(0.0, 0.0));

    assert!(first.items.is_empty() && second.items.is_empty());
    assert_eq!(
        raster.count(),
        2,
        "each answer is remembered after one raster"
    );
}

#[test]
fn whole_pixel_moves_keep_pixels_and_fractional_moves_do_not() {
    let at = |x: f32, y: f32| Matrix4::translation(x, y, 0.0);
    assert!(keeps_pixels(&at(0.25, 1.0), &at(3.25, -4.0)));
    assert!(!keeps_pixels(&at(0.25, 1.0), &at(0.5, 1.0)));
    assert!(!keeps_pixels(&at(0.0, 0.0), &Matrix4::scale(2.0, 2.0, 1.0)));
}
