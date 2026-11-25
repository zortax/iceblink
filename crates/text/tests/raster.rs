//! Glyph bitmaps: the three layouts, phases, hinting, colour, variations and batches.

mod common;

use iceblink::GlyphRaster;
use iceblink::{FontData, GlyphBitmap, GlyphKey, GlyphPlacement, PHASES, RasterStyle};
use iceblink_text::SwashRaster;
use skrifa::MetadataProvider;
use skrifa::raw::TableProvider;

use crate::common::{COLOR, LATIN, VARIABLE, font, glyph, ink, key};

/// Rasterises `key` of the font in `file` at the default instance.
fn raster(file: &str, key: &GlyphKey) -> (Option<GlyphPlacement>, GlyphBitmap) {
    let mut bitmap = GlyphBitmap::default();
    let placement = SwashRaster::new().rasterize(&font(file), key, &[], &mut bitmap);
    (placement, bitmap)
}

#[test]
fn every_style_has_its_bitmap_layout() {
    let latin = font(LATIN);
    let g = glyph(&latin, 'g');
    for style in [RasterStyle::Mono, RasterStyle::Subpixel, RasterStyle::Color] {
        let (placement, bitmap) = raster(LATIN, &key(g, 32.0, style));
        assert!(placement.is_some(), "{style:?}");
        assert!(!bitmap.is_empty(), "{style:?}");
        assert!(bitmap.is_well_formed(style), "{style:?}");
    }
}

#[test]
fn equal_keys_give_identical_bitmaps() {
    let latin = font(LATIN);
    let raster = SwashRaster::new();
    let key = key(glyph(&latin, 'g'), 32.0, RasterStyle::Mono);
    let (mut first, mut second) = (GlyphBitmap::default(), GlyphBitmap::default());
    let placements = (
        raster.rasterize(&latin, &key, &[], &mut first),
        raster.rasterize(&latin, &key, &[], &mut second),
    );
    assert_eq!(placements.0, placements.1);
    assert_eq!(first, second);
}

/// Returns where the coverage of one channel of a subpixel bitmap sits along x, in pixels.
fn channel_centre(bitmap: &GlyphBitmap, channel: usize) -> f32 {
    let (mut weight, mut moment) = (0.0, 0.0);
    for (index, pixel) in bitmap.bytes.as_chunks::<3>().0.iter().enumerate() {
        let x = (index as u32 % bitmap.width) as f32;
        weight += f32::from(pixel[channel]);
        moment += f32::from(pixel[channel]) * x;
    }
    moment / weight
}

#[test]
fn subpixel_channels_are_in_red_to_blue_stripe_order() {
    // The red stripe sees the outline a third of a pixel further right than green, and blue a
    // third further left, so red's ink sits two thirds of a pixel right of blue's.
    let latin = font(LATIN);
    let (_, bitmap) = raster(LATIN, &key(glyph(&latin, 'l'), 32.0, RasterStyle::Subpixel));
    let lead = channel_centre(&bitmap, 0) - channel_centre(&bitmap, 2);
    assert!(
        (0.5..0.8).contains(&lead),
        "red leads blue by {lead} pixels"
    );
}

#[test]
fn each_phase_is_a_different_rasterisation() {
    let latin = font(LATIN);
    let n = glyph(&latin, 'n');
    let at = |phase: u8| {
        raster(
            LATIN,
            &GlyphKey {
                phase,
                ..key(n, 24.0, RasterStyle::Mono)
            },
        )
        .1
    };
    let bitmaps: Vec<GlyphBitmap> = (0..PHASES).map(at).collect();
    for (first, a) in bitmaps.iter().enumerate() {
        for b in &bitmaps[first + 1..] {
            assert_ne!(a, b, "two phases rasterised alike");
        }
    }
}

#[test]
fn hinting_is_taken_from_the_key() {
    let latin = font(LATIN);
    let unhinted = key(glyph(&latin, 'e'), 11.0, RasterStyle::Mono);
    let hinted = GlyphKey {
        hint: true,
        ..unhinted
    };
    assert_ne!(raster(LATIN, &unhinted).1, raster(LATIN, &hinted).1);
}

#[test]
fn a_colour_glyph_is_straight_colour_from_its_layers() {
    let bitmap = (1..64u32)
        .find_map(|id| {
            let (placement, bitmap) = raster(COLOR, &key(id, 48.0, RasterStyle::Color));
            placement?;
            let coloured = bitmap
                .bytes
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] > 0 && pixel[..3] != [255, 255, 255]);
            coloured.then_some(bitmap)
        })
        .expect("the colour face draws a glyph in colour");
    assert!(bitmap.is_well_formed(RasterStyle::Color));
    let straight =
        bitmap.bytes.as_chunks::<4>().0.iter().any(|pixel| {
            pixel[3] > 0 && pixel[3] < 255 && pixel[..3].iter().any(|c| *c > pixel[3])
        });
    assert!(straight, "partly covered texels keep their full colour");
}

#[test]
fn a_colour_key_of_a_plain_face_is_white_coverage() {
    let latin = font(LATIN);
    let (_, bitmap) = raster(LATIN, &key(glyph(&latin, 'o'), 32.0, RasterStyle::Color));
    assert!(bitmap.is_well_formed(RasterStyle::Color));
    assert!(
        bitmap
            .bytes
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[..3] == [255, 255, 255])
    );
    assert!(
        bitmap
            .bytes
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] == 255)
    );
}

#[test]
fn a_blank_glyph_is_empty_and_a_missing_one_is_none() {
    let latin = font(LATIN);
    let (placement, bitmap) = raster(LATIN, &key(glyph(&latin, ' '), 16.0, RasterStyle::Mono));
    assert!(placement.is_some());
    assert!(bitmap.is_empty() && bitmap.bytes.is_empty());

    let count = u32::from(
        skrifa::FontRef::new(latin.bytes())
            .and_then(|face| face.maxp())
            .expect("parses")
            .num_glyphs(),
    );
    assert_eq!(raster(LATIN, &key(count, 16.0, RasterStyle::Mono)).0, None);
    assert_eq!(raster(LATIN, &key(70_000, 16.0, RasterStyle::Mono)).0, None);
}

#[test]
fn a_glyph_of_an_unreadable_font_is_none_and_blank() {
    let latin = font(LATIN);
    let key = key(glyph(&latin, 'g'), 32.0, RasterStyle::Mono);
    let raster = SwashRaster::new();
    let mut bitmap = GlyphBitmap::default();
    raster.rasterize(&latin, &key, &[], &mut bitmap);
    assert!(!bitmap.is_empty());

    let unreadable = FontData::new(vec![0_u8; 64], 0);

    assert_eq!(raster.rasterize(&unreadable, &key, &[], &mut bitmap), None);
    assert!(bitmap.is_empty() && bitmap.bytes.is_empty());
}

#[test]
fn a_variable_face_draws_at_its_coordinates() {
    let variable = font(VARIABLE);
    let face = skrifa::FontRef::new(variable.bytes()).expect("parses");
    let bold: Vec<i16> = face
        .axes()
        .location([("wght", 700.0)])
        .coords()
        .iter()
        .map(|coord| coord.to_bits())
        .collect();
    let key = key(glyph(&variable, 'H'), 48.0, RasterStyle::Mono);

    let raster = SwashRaster::new();
    let (mut regular, mut heavy) = (GlyphBitmap::default(), GlyphBitmap::default());
    raster.rasterize(&variable, &key, &[], &mut regular);
    let instanced = GlyphKey { instance: 1, ..key };
    raster.rasterize(&variable, &instanced, &bold, &mut heavy);
    assert!(
        ink(&heavy) > ink(&regular) * 11 / 10,
        "700 drew no heavier than 400: {} against {}",
        ink(&heavy),
        ink(&regular)
    );
}

#[test]
fn a_slant_leans_the_bitmap_forwards() {
    let latin = font(LATIN);
    let slanted = GlyphKey {
        skew_bits: 14.0f32.to_bits(),
        ..key(glyph(&latin, 'l'), 24.0, RasterStyle::Mono)
    };
    let (_, bitmap) = raster(LATIN, &slanted);
    let column = |rows: std::ops::Range<u32>| {
        let (mut sum, mut weight) = (0.0f32, 0.0f32);
        for row in rows {
            for x in 0..bitmap.width {
                let coverage = f32::from(bitmap.bytes[(row * bitmap.width + x) as usize]);
                sum += coverage * x as f32;
                weight += coverage;
            }
        }
        sum / weight.max(1.0)
    };
    let quarter = bitmap.height / 4;
    let (top, foot) = (
        column(0..quarter),
        column(bitmap.height - quarter..bitmap.height),
    );
    assert!(
        top > foot + 2.0,
        "the top sits at {top} and the foot at {foot}"
    );
}

#[test]
fn a_batch_answers_what_single_calls_answer_on_any_thread() {
    let latin = font(LATIN);
    let ids: Vec<u32> = "the quick brown fox 0123"
        .chars()
        .map(|c| glyph(&latin, c))
        .collect();
    let mut keys = Vec::new();
    for size in [11.0, 13.5, 24.0] {
        for phase in 0..PHASES {
            for style in [RasterStyle::Mono, RasterStyle::Subpixel, RasterStyle::Color] {
                for hint in [false, true] {
                    for id in &ids {
                        keys.push(GlyphKey {
                            phase,
                            hint,
                            ..key(*id, size, style)
                        });
                    }
                }
            }
        }
    }

    let raster = SwashRaster::new();
    let single: Vec<(Option<GlyphPlacement>, GlyphBitmap)> = keys
        .iter()
        .map(|key| {
            let mut bitmap = GlyphBitmap::default();
            (raster.rasterize(&latin, key, &[], &mut bitmap), bitmap)
        })
        .collect();

    let batches: Vec<Vec<GlyphKey>> = keys
        .chunks(keys.len().div_ceil(4))
        .map(|chunk| chunk.iter().rev().copied().collect())
        .collect();
    std::thread::scope(|scope| {
        for batch in &batches {
            let (raster, latin, keys, single) = (&raster, &latin, &keys, &single);
            scope.spawn(move || {
                raster.rasterize_many(latin, &[], batch, &mut |index, placement, bitmap| {
                    let at = keys
                        .iter()
                        .position(|key| *key == batch[index])
                        .expect("a key of the batch");
                    assert_eq!((placement, bitmap), (single[at].0, &single[at].1));
                });
            });
        }
    });
}

#[test]
fn a_glyph_with_colour_layers_is_a_colour_glyph() {
    let raster = SwashRaster::new();
    let color = font(COLOR);
    assert!((1..64).any(|id| raster.is_color(&color, id)));
    let latin = font(LATIN);
    assert!(!raster.is_color(&latin, glyph(&latin, 'g')));
}

/// Returns the pixels with coverage of a mono bitmap at `placement`, as `[left, top, right,
/// bottom]` from the glyph's origin with y down.
fn covered(bitmap: &GlyphBitmap, placement: GlyphPlacement) -> [i32; 4] {
    let width = bitmap.width as usize;
    let mut found = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
    for (index, _) in bitmap
        .bytes
        .iter()
        .enumerate()
        .filter(|(_, byte)| **byte != 0)
    {
        let x = placement.left + (index % width) as i32;
        let y = -placement.top + (index / width) as i32;
        found = [
            found[0].min(x),
            found[1].min(y),
            found[2].max(x + 1),
            found[3].max(y + 1),
        ];
    }
    found
}

#[test]
fn emboldening_grows_every_side_by_its_pixels() {
    let latin = font(LATIN);
    let stem = key(glyph(&latin, 'l'), 24.0, RasterStyle::Mono);
    let mut bold = stem;
    bold.embolden_bits = 1.0f32.to_bits();

    let (placement, plain) = raster(LATIN, &stem);
    let (bold_placement, bold) = raster(LATIN, &bold);

    let [left, top, right, bottom] = covered(&plain, placement.expect("the font has an l"));
    assert_eq!(
        covered(&bold, bold_placement.expect("the font has an l")),
        [left - 1, top - 1, right + 1, bottom + 1],
        "an emboldening of one pixel adds a pixel on every side of a plain stem"
    );
}
