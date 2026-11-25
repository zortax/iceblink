//! Which sprite or path a glyph lowers into.

mod common;

use common::content::{
    FIRST_COLOR, FakeRaster, NO_OUTLINE, cache_with, drain, glyph, gradient, input, lower, moved,
    run, solid, upload_bytes,
};
use iceblink::geom::Affine2;
use iceblink::{GlyphFlags, Matrix4, PageFormat, PageOp, PenPosition, RasterStyle, SpriteFlags};
use iceblink_core::content::{Lowered, Target};

#[test]
fn colour_glyphs_carry_the_run_paint_with_paint_alpha_and_full_opacity() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let run = run(16.0, solid(9), GlyphFlags::default());
    let glyphs = [glyph(FIRST_COLOR, 2.0, 10.0), glyph(5, 12.0, 10.0)];

    let lowering = lower(&mut cache, &input(&run, &glyphs), &moved(0.0, 0.0));

    match &lowering.items[0] {
        Lowered::Color(sprite) => {
            assert_eq!(sprite.flags, SpriteFlags::PAINT_ALPHA);
            assert_eq!(sprite.paint, solid(9));
            assert_eq!(sprite.opacity, 1.0);
            assert_eq!(sprite.frame, sprite.bounds);
            assert_eq!(sprite.radii, [0.0; 8]);
        }
        other => panic!("expected a colour sprite, got {other:?}"),
    }
    assert!(matches!(lowering.items[1], Lowered::Mono(_)));
    assert_eq!(raster.keys()[0].style, RasterStyle::Color);

    let ops = drain(&mut cache);
    let colour_page = ops.iter().find_map(|op| match op {
        PageOp::Create { format, .. } if *format == PageFormat::RGBA8_PREMULTIPLIED => Some(()),
        _ => None,
    });
    assert!(
        colour_page.is_some(),
        "colour glyphs live on a premultiplied page"
    );
    let premultiplied: Vec<u8> = ops
        .iter()
        .filter_map(upload_bytes)
        .find(|bytes| bytes.len() >= 4 && bytes[3] == 128)
        .map(|bytes| bytes[..4].to_vec())
        .expect("the colour glyph's texels");
    assert_eq!(premultiplied, [100, 50, 25, 128]);
}

#[test]
fn a_silhouette_draws_plain_glyphs_and_drops_colour_glyphs() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let run = run(
        16.0,
        solid(1),
        GlyphFlags::SILHOUETTE | GlyphFlags::SUBPIXEL,
    );
    let glyphs = [glyph(FIRST_COLOR, 0.0, 10.0), glyph(5, 10.0, 10.0)];
    let mut input = input(&run, &glyphs);
    input.subpixel = true;

    let lowering = lower(&mut cache, &input, &moved(0.0, 0.0));

    assert_eq!(lowering.items.len(), 1);
    assert!(
        matches!(lowering.items[0], Lowered::Mono(_)),
        "a silhouette has no stripes"
    );
    assert!(
        raster
            .keys()
            .iter()
            .all(|key| key.style == RasterStyle::Mono)
    );
}

#[test]
fn subpixel_sprites_need_the_run_flag_and_the_output_permission() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let glyphs = [glyph(5, 0.0, 10.0)];
    let flagged = run(16.0, solid(1), GlyphFlags::SUBPIXEL);
    let plain = run(16.0, solid(1), GlyphFlags::default());

    let mut allowed = input(&flagged, &glyphs);
    allowed.subpixel = true;
    let mut unflagged = input(&plain, &glyphs);
    unflagged.subpixel = true;

    let kinds: Vec<bool> = [allowed, unflagged, input(&flagged, &glyphs)]
        .iter()
        .map(|input| {
            matches!(
                lower(&mut cache, input, &moved(0.0, 0.0)).items[0],
                Lowered::Subpixel(_)
            )
        })
        .collect();
    assert_eq!(kinds, [true, false, false]);
}

#[test]
fn large_turned_and_gradient_runs_take_the_outline_route() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let glyphs = [glyph(5, 1.0, 10.0)];
    let small = run(16.0, solid(1), GlyphFlags::default());
    let large = run(97.0, solid(1), GlyphFlags::default());
    let painted = run(16.0, gradient(1), GlyphFlags::default());
    let turned = Target::new(Affine2::rotation(0.3).to_matrix4());

    let is_path = |cache: &mut iceblink_core::content::ContentCache, run, target: &Target| {
        matches!(
            lower(cache, &input(run, &glyphs), target).items[0],
            Lowered::Path(_)
        )
    };
    assert!(!is_path(&mut cache, &small, &moved(0.0, 0.0)));
    assert!(is_path(&mut cache, &large, &moved(0.0, 0.0)));
    assert!(is_path(&mut cache, &painted, &moved(0.0, 0.0)));
    assert!(is_path(&mut cache, &small, &turned));

    let mut dense = input(&large, &glyphs);
    dense.output_scale = 2.0;
    assert!(
        !matches!(
            lower(&mut cache, &dense, &moved(0.0, 0.0)).items[0],
            Lowered::Path(_)
        ),
        "97 logical pixels at scale 2 stay below the device limit"
    );
}

#[test]
fn an_outlined_glyph_is_filled_and_stroked_by_the_emboldening() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let mut bold = run(120.0, solid(4), GlyphFlags::default());
    bold.embolden = 1.5;
    let glyphs = [glyph(5, 10.0, 100.0), glyph(5, 70.0, 100.0)];

    let lowering = lower(&mut cache, &input(&bold, &glyphs), &moved(2.0, 0.0));

    let paths: Vec<_> = lowering
        .items
        .iter()
        .map(|item| match item {
            Lowered::Path(path) => path.clone(),
            other => panic!("expected a path, got {other:?}"),
        })
        .collect();
    assert!(
        std::sync::Arc::ptr_eq(&paths[0].path, &paths[1].path),
        "one outline per key"
    );
    assert_eq!(paths[0].fill.unwrap().paint, solid(4));
    assert_eq!(paths[0].stroke.as_ref().unwrap().style.width, 3.0);
    assert_eq!(paths[0].transform.translation().x, 12.0);
    assert!(paths[0].ink.origin.x <= 12 && paths[0].ink.size.width > 0);
    assert_eq!(raster.outlines.lock().unwrap().len(), 1);
    assert!(lowering.tiles.is_empty());
}

#[test]
fn the_emboldening_grows_each_side_by_its_device_pixels_under_a_scale() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let mut bold = run(120.0, solid(4), GlyphFlags::default());
    bold.embolden = 1.5;
    let glyphs = [glyph(5, 10.0, 100.0)];
    let scaled = Target::new(iceblink::Matrix4::scale(4.0, 4.0, 1.0));

    let lowering = lower(&mut cache, &input(&bold, &glyphs), &scaled);

    let Lowered::Path(path) = &lowering.items[0] else {
        panic!("expected a path, got {:?}", lowering.items[0]);
    };
    let local = path.stroke.as_ref().unwrap().style.width;
    let device = local * path.transform.determinant().abs().sqrt();
    assert!(
        (device - 3.0).abs() < 1e-9,
        "the stroke is {device} device pixels wide, twice the emboldening"
    );
}

#[test]
fn a_glyph_without_an_outline_falls_back_to_a_resampled_tile() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let run = run(16.0, solid(1), GlyphFlags::default());
    let glyphs = [glyph(NO_OUTLINE, 3.0, 10.0)];
    let turned = Target::new(Affine2::rotation(0.3).to_matrix4());

    let lowering = lower(&mut cache, &input(&run, &glyphs), &turned);

    assert!(matches!(lowering.items[0], Lowered::Mono(_)));
    assert_eq!(raster.keys()[0].phase, 0);
    assert_eq!(raster.keys()[0].size(), 16.0);
}

#[test]
fn a_moving_slight_scale_keeps_resampled_tiles_and_a_resting_one_rasterises_exactly() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let run = run(16.0, solid(1), GlyphFlags::default());
    let glyphs = [glyph(5, 3.0, 10.0)];
    let mut target = Target::new(Matrix4::scale(0.9, 0.9, 1.0));
    target.moving = true;

    let moving = lower(&mut cache, &input(&run, &glyphs), &target);
    target.moving = false;
    let resting = lower(&mut cache, &input(&run, &glyphs), &target);

    assert!(matches!(moving.items[0], Lowered::Mono(_)));
    assert!(matches!(resting.items[0], Lowered::Mono(_)));
    let sizes: Vec<f32> = raster.keys().iter().map(|key| key.size()).collect();
    assert_eq!(sizes, [16.0, 16.0 * 0.9]);
}

#[test]
fn a_colour_glyph_of_an_outlined_run_still_lands_on_whole_device_pixels() {
    let raster = FakeRaster::shared();
    let mut cache = cache_with(&raster);
    let large = run(120.0, gradient(1), GlyphFlags::default());
    let glyphs = [glyph(FIRST_COLOR, 3.3, 100.0)];

    let lowering = lower(&mut cache, &input(&large, &glyphs), &moved(0.5, 0.25));

    match &lowering.items[..] {
        [Lowered::Color(sprite)] => {
            assert_eq!((sprite.bounds[0] + 0.5).fract(), 0.0);
            assert_eq!((sprite.bounds[1] + 0.25).fract(), 0.0);
        }
        other => panic!("expected one colour sprite, got {other:?}"),
    }
    assert_eq!(raster.keys()[0].phase, PenPosition::of(3.8).phase());
}
