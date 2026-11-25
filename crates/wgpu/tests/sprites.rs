//! Sprites over atlas pages: colour sprites that sample a level of detail when drawn smaller, and
//! mono sprites corrected for contrast and gamma.

mod common;

use std::sync::Arc;

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{plain_renderer, present};
use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    AtlasTile, ColorSprite, FRAME_NO_CLIP, PageFormat, PageOp, PaintRef, ROUND, Sprite, SpriteFlags,
};

/// Adds the creation of page `page` of `format` and `size`, and an upload that repeats `texel`
/// over all of it.
fn page(builder: &mut FrameBuilder, page: u32, format: PageFormat, size: u32, texel: &[u8]) {
    builder.page(PageOp::Create {
        page,
        format,
        size: [size, size],
        levels: 1,
    });
    let bytes: Vec<u8> = texel
        .iter()
        .copied()
        .cycle()
        .take((size * size) as usize * texel.len())
        .collect();
    builder.page(PageOp::Upload {
        page,
        level: 0,
        rect: Rect::new(Point::new(0, 0), Size::new(size as i32, size as i32)),
        bytes: Arc::from(bytes),
        offset: 0,
        stride: size * texel.len() as u32,
    });
}

/// Returns a colour sprite of the 8 by 8 tile at the origin of page 1.
fn color_sprite(
    bounds: [f32; 4],
    frame: [f32; 4],
    paint: PaintRef,
    flags: SpriteFlags,
) -> ColorSprite {
    ColorSprite {
        bounds,
        frame,
        radii: [0.0; 8],
        shapes: ROUND,
        tile: AtlasTile {
            page: 1,
            rect: [0, 0, 8, 8],
        },
        paint,
        paint_origin: [0.0; 2],
        opacity: 1.0,
        clip: FRAME_NO_CLIP,
        space: 0,
        flags,
    }
}

#[test]
fn a_colour_sprite_drawn_at_a_quarter_of_its_size_samples_the_level_of_that_size() {
    // Level 0 red, level 1 green, level 2 blue, then yellow.
    let colours = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
    let draw = |levels: u32| {
        let mut renderer = plain_renderer();
        let mut builder = FrameBuilder::new();
        builder.page(PageOp::Create {
            page: 1,
            format: PageFormat::RGBA8_PREMULTIPLIED,
            size: [64, 64],
            levels,
        });
        for level in 0..levels {
            let [width, height] = PageOp::level_size([64, 64], level);
            let texel = colours
                .get(level as usize)
                .copied()
                .unwrap_or([255, 255, 0, 255]);
            let bytes: Vec<u8> = texel
                .into_iter()
                .cycle()
                .take((width * height * 4) as usize)
                .collect();
            builder.page(PageOp::Upload {
                page: 1,
                level,
                rect: Rect::new(Point::new(0, 0), Size::new(width as i32, height as i32)),
                bytes: Arc::from(bytes),
                offset: 0,
                stride: width * 4,
            });
        }
        let white = builder.solid(opaque(255, 255, 255));
        let mut sprite = color_sprite(
            rect(0.0, 0.0, 16.0, 16.0),
            rect(0.0, 0.0, 16.0, 16.0),
            white,
            SpriteFlags::default(),
        );
        sprite.tile.rect = [0, 0, 64, 64];
        builder.color(sprite);
        present(&mut renderer, &builder.finish()).rgba(8, 8)
    };

    assert_eq!(
        draw(1),
        [255, 0, 0, 255],
        "a page of one level samples level 0"
    );
    let sampled = draw(7);
    let near = |a: u8, b: u8| a.abs_diff(b) <= 2;
    assert!(
        near(sampled[0], 0) && near(sampled[1], 0) && near(sampled[2], 255),
        "four texels per pixel sample level 2: {sampled:?}"
    );
}

/// Returns the mono correction of coverage `c` under the straight grey `level`, by the rule of
/// `Sprite`.
fn corrected_coverage(c: f32, level: f32) -> f32 {
    let contrast = 0.5 * (4.0 * (0.75 - level)).clamp(0.0, 1.0);
    let enhanced = c * (contrast + 1.0) / (c * contrast + 1.0);
    let [g0, g1, g2, g3] = iceblink::color::gamma_correction_ratios(2.2);
    let correction = (g0 * level + g1) * enhanced + g2 * level + g3;
    enhanced + enhanced * (1.0 - enhanced) * correction
}

#[test]
fn mono_sprites_are_corrected_for_contrast_and_gamma() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    page(&mut builder, 0, PageFormat::A8, 16, &[128]);
    let black = builder.solid(opaque(0, 0, 0));
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled(rect(0.0, 0.0, 128.0, 64.0), black));
    builder.quad(filled(rect(0.0, 64.0, 128.0, 64.0), white));
    let mask = |x: f32, y: f32, paint: PaintRef| Sprite {
        bounds: rect(x, y, 16.0, 16.0),
        paint,
        paint_origin: [0.0; 2],
        tile: AtlasTile {
            page: 0,
            rect: [0, 0, 16, 16],
        },
        clip: FRAME_NO_CLIP,
        space: 0,
        _pad: 0,
    };
    builder.mono(mask(8.0, 8.0, white));
    builder.mono(mask(8.0, 72.0, black));
    let pixels = present(&mut renderer, &builder.finish());

    let coverage = 128.0 / 255.0;
    let byte = |value: f32| (value * 255.0).round() as u8;
    let near = |found: u8, expected: u8| found.abs_diff(expected) <= 1;
    let light = pixels.rgba(16, 16)[0];
    assert!(
        near(light, byte(corrected_coverage(coverage, 1.0))),
        "white on black draws {light}"
    );
    let dark = pixels.rgba(16, 80)[0];
    assert!(
        near(dark, byte(1.0 - corrected_coverage(coverage, 0.0))),
        "black on white draws {dark}"
    );
}
