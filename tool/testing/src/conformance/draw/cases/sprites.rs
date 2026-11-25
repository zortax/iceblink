//! Coverage sprites of both kinds and colour sprites in every mode.

use std::sync::Arc;

use iceblink::{
    AtlasTile, ColorSprite, FRAME_NO_CLIP, FrameGradient, FramePaint, FrameStop, GradientShape,
    Matrix4, PageFormat, PaintRef, ROUND, Sprite, SpriteFlags, StopEncoding,
};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, filled, opaque, straight};
use crate::conformance::draw::canvas::{Canvas, SIZE, golden, page, skip};
use crate::conformance::draw::cases::cell;
use crate::golden::Tolerance;

/// Sampled texels: a pixel may differ by a few steps where it blends texels or corrects coverage.
const TEXELS: Tolerance = Tolerance {
    max_channel: 3,
    max_pixels: 32,
};

/// The side of the tiles the cases upload.
const TILE: u32 = 32;

/// Coverage that ramps up from left to right in the top half and fills a disc in the bottom half.
fn coverage() -> Vec<u8> {
    let mut bytes = Vec::with_capacity((TILE * TILE) as usize);
    for y in 0..TILE {
        for x in 0..TILE {
            let value = if y < TILE / 2 {
                (x * 255 / (TILE - 1)) as u8
            } else {
                let dx = x as f32 + 0.5 - 16.0;
                let dy = y as f32 + 0.5 - 24.0;
                (((7.5 - (dx * dx + dy * dy).sqrt()) * 255.0).clamp(0.0, 255.0)) as u8
            };
            bytes.push(value);
        }
    }
    bytes
}

fn coverage_sprite(origin: [f32; 2], paint: PaintRef) -> Sprite {
    Sprite {
        bounds: [origin[0], origin[1], TILE as f32, TILE as f32],
        paint,
        paint_origin: [0.0; 2],
        tile: AtlasTile {
            page: 0,
            rect: [0, 0, TILE, TILE],
        },
        clip: FRAME_NO_CLIP,
        space: 0,
        _pad: 0,
    }
}

/// Mono sprites light on dark and dark on light, a sprite filled with a gradient
/// and a sprite resampled under a scale.
pub fn coverage_sprites(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    page(&mut builder, 0, PageFormat::A8, [TILE, TILE], coverage());
    let black = builder.solid(opaque(0, 0, 0));
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, 128.0, 64.0], black));
    builder.quad(filled([0.0, 64.0, 128.0, 64.0], white));
    let ramp = builder.paint(FramePaint::Gradient(FrameGradient {
        shape: GradientShape::Linear {
            start: [84.0, 0.0],
            end: [116.0, 0.0],
        },
        encoding: StopEncoding::SRGB,
        repeat: false,
        stops: Arc::from(vec![
            FrameStop {
                color: [1.0, 0.8, 0.0, 1.0],
                offset: 0.0,
            },
            FrameStop {
                color: [0.0, 0.8, 1.0, 1.0],
                offset: 1.0,
            },
        ]),
    }));

    builder.mono(coverage_sprite([4.0, 4.0], white));
    builder.mono(coverage_sprite([44.0, 4.0], white));
    builder.mono(coverage_sprite([84.0, 4.0], ramp));
    builder.mono(coverage_sprite([4.0, 68.0], black));
    builder.mono(coverage_sprite([44.0, 68.0], black));
    let scaled = builder.space(Matrix4::scale(1.25, 1.25, 1.0));
    let mut sprite = coverage_sprite([66.0, 54.0], black);
    sprite.space = scaled;
    builder.mono(sprite);

    golden("coverage_sprites", &canvas.draw(builder.finish()), TEXELS);
}

/// Four quadrants of red, green, blue and half-transparent white, premultiplied.
fn quadrants() -> Vec<u8> {
    let mut bytes = Vec::with_capacity((TILE * TILE * 4) as usize);
    for y in 0..TILE {
        for x in 0..TILE {
            let texel = match (x < TILE / 2, y < TILE / 2) {
                (true, true) => [255, 0, 0, 255],
                (false, true) => [0, 255, 0, 255],
                (true, false) => [0, 0, 255, 255],
                (false, false) => [128, 128, 128, 128],
            };
            bytes.extend_from_slice(&texel);
        }
    }
    bytes
}

fn colour_sprite(bounds: [f32; 4], flags: SpriteFlags) -> ColorSprite {
    ColorSprite {
        bounds,
        frame: bounds,
        radii: [0.0; 8],
        shapes: ROUND,
        tile: AtlasTile {
            page: 1,
            rect: [0, 0, TILE, TILE],
        },
        paint: PaintRef::NONE,
        paint_origin: [0.0; 2],
        opacity: 1.0,
        clip: FRAME_NO_CLIP,
        space: 0,
        flags,
    }
}

/// Colour sprites: stretched, cut to a rounded frame, desaturated, used as a mask, repeated,
/// sampled at the nearest texel, scaled by a paint's alpha, faded, and cut to a smaller frame.
pub fn colour_sprites(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    page(
        &mut builder,
        1,
        PageFormat::RGBA8_PREMULTIPLIED,
        [TILE, TILE],
        quadrants(),
    );
    let grey = builder.solid(opaque(200, 200, 200));
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], grey));
    let green = builder.solid(opaque(0, 160, 60));
    let faint = builder.solid(straight(0, 0, 0, 96));

    builder.color(colour_sprite(cell(0), SpriteFlags::default()));
    builder.color(ColorSprite {
        radii: [10.0; 8],
        ..colour_sprite(cell(1), SpriteFlags::default())
    });
    builder.color(colour_sprite(cell(2), SpriteFlags::GRAYSCALE));
    builder.color(ColorSprite {
        paint: green,
        ..colour_sprite(cell(3), SpriteFlags::MASK)
    });
    let [x, y, width, height] = cell(4);
    builder.color(ColorSprite {
        frame: [x, y, width, height],
        ..colour_sprite(
            [x, y, 12.0, 12.0],
            SpriteFlags::REPEAT_X | SpriteFlags::REPEAT_Y,
        )
    });
    builder.color(colour_sprite(cell(5), SpriteFlags::NEAREST));
    builder.color(ColorSprite {
        paint: faint,
        ..colour_sprite(cell(6), SpriteFlags::PAINT_ALPHA)
    });
    builder.color(ColorSprite {
        opacity: 0.5,
        ..colour_sprite(cell(7), SpriteFlags::default())
    });
    let [x, y, width, height] = cell(8);
    builder.color(ColorSprite {
        frame: [x + 6.0, y + 6.0, width - 12.0, height - 12.0],
        radii: [4.0; 8],
        ..colour_sprite(cell(8), SpriteFlags::default())
    });

    golden("colour_sprites", &canvas.draw(builder.finish()), TEXELS);
}

/// Per-stripe coverage: the disc and ramp of [`coverage`] with the red stripe a third of a pixel
/// ahead of green and blue a third behind.
fn stripes() -> Vec<u8> {
    let single = coverage();
    let mut bytes = Vec::with_capacity(single.len() * 4);
    for (index, &value) in single.iter().enumerate() {
        let x = index as u32 % TILE;
        let next = if x + 1 < TILE { single[index + 1] } else { 0 };
        let previous = if x > 0 { single[index - 1] } else { 0 };
        let red = ((u32::from(value) * 2 + u32::from(next)) / 3) as u8;
        let blue = ((u32::from(value) * 2 + u32::from(previous)) / 3) as u8;
        bytes.extend_from_slice(&[red, value, blue, 0]);
    }
    bytes
}

/// Subpixel glyph masks in black and in colour over white.
pub fn subpixel_sprites(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    if !canvas.caps().subpixel_text {
        skip("subpixel_sprites", "the renderer draws no subpixel text");
        return;
    }
    let mut builder = FrameBuilder::new(SIZE);
    page(
        &mut builder,
        0,
        PageFormat::SUBPIXEL,
        [TILE, TILE],
        stripes(),
    );
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], white));
    let black = builder.solid(opaque(0, 0, 0));
    let teal = builder.solid(opaque(0, 120, 140));
    builder.subpixel(coverage_sprite([8.0, 8.0], black));
    builder.subpixel(coverage_sprite([48.0, 8.0], teal));
    golden("subpixel_sprites", &canvas.draw(builder.finish()), TEXELS);
}
