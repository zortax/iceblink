//! Subpixel glyphs drawn from the bitmaps of the glyph rasteriser.

mod common;

use std::sync::Arc;

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{SIDE, plain_renderer, present};
use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    AtlasTile, FRAME_NO_CLIP, FontId, GlyphBitmap, GlyphKey, PageFormat, PageOp, PenPosition,
    RasterStyle, Sprite,
};
use iceblink::{GlyphRaster, Renderer as _};
use iceblink_text::SwashRaster;
use iceblink_wgpu::Pixels;
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size as FontSize};

/// The side of the page the glyphs are placed on, in texels.
const PAGE: u32 = 64;

/// The size the glyphs are drawn at, in device pixels.
const EM: f32 = 24.0;

/// Draws `text` in white subpixel glyphs on black, with the pen starting at `origin`, and returns
/// the image.
///
/// Each glyph is rasterised at the phase of its position and drawn on whole pixels, as the
/// compositor places atlas glyphs.
fn drawn(text: &str, origin: [f32; 2]) -> Pixels {
    let font = iceblink_testing::fonts::read_font(iceblink_testing::fonts::LATIN);
    let face = skrifa::FontRef::new(font.bytes()).expect("the test font parses");
    let charmap = face.charmap();
    let metrics = face.glyph_metrics(FontSize::new(EM), LocationRef::default());
    let raster = SwashRaster::new();

    let mut builder = FrameBuilder::new();
    let black = builder.solid(opaque(0, 0, 0));
    builder.quad(filled(rect(0.0, 0.0, SIDE as f32, SIDE as f32), black));
    let white = builder.solid(opaque(255, 255, 255));
    builder.page(PageOp::Create {
        page: 0,
        format: PageFormat::SUBPIXEL,
        size: [PAGE, PAGE],
        levels: 1,
    });

    let mut x = origin[0];
    let mut column = 0;
    let mut bitmap = GlyphBitmap::default();
    for character in text.chars() {
        let id = charmap
            .map(character)
            .expect("the test font has the letter");
        let pen = PenPosition::of(x);
        x += metrics
            .advance_width(id)
            .expect("the letter has an advance");
        let key = GlyphKey {
            font: FontId(0),
            glyph: id.to_u32(),
            size_bits: EM.to_bits(),
            instance: 0,
            phase: pen.phase(),
            style: RasterStyle::Subpixel,
            hint: true,
            palette: 0,
            embolden_bits: 0,
            skew_bits: 0,
        };
        let placement = raster
            .rasterize(&font, &key, &[], &mut bitmap)
            .expect("the letter rasterises");
        let [width, height] = [bitmap.width, bitmap.height];
        let texels: Vec<u8> = bitmap
            .bytes
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], u8::MAX])
            .collect();
        builder.page(PageOp::Upload {
            page: 0,
            level: 0,
            rect: Rect::new(
                Point::new(column as i32, 0),
                Size::new(width as i32, height as i32),
            ),
            bytes: Arc::from(texels),
            offset: 0,
            stride: width * 4,
        });
        let [left, top] = pen.bitmap_origin(origin[1], placement);
        builder.subpixel(Sprite {
            bounds: rect(left, top, width as f32, height as f32),
            paint: white,
            paint_origin: [0.0; 2],
            tile: AtlasTile {
                page: 0,
                rect: [column, 0, width, height],
            },
            clip: FRAME_NO_CLIP,
            space: 0,
            _pad: 0,
        });
        column += width;
    }

    let mut renderer = plain_renderer();
    assert!(
        renderer.capabilities().subpixel_text,
        "the device blends per-channel coverage"
    );
    present(&mut renderer, &builder.finish())
}

/// Returns how far the centre of the red ink lies right of the centre of the blue ink, in pixels.
fn red_lead(pixels: &Pixels) -> f32 {
    let centre = |channel: usize| {
        let (mut weight, mut moment) = (0.0, 0.0);
        for y in 0..SIDE as i32 {
            for x in 0..SIDE as i32 {
                let ink = f64::from(pixels.rgba(x, y)[channel]);
                weight += ink;
                moment += ink * f64::from(x);
            }
        }
        moment / weight
    };
    (centre(0) - centre(2)) as f32
}

// The red stripe of a pixel lies a third of a pixel left of its centre and the blue stripe a third
// right, so the red ink of white text sits about two thirds of a pixel right of the blue ink. The
// `g` lands at a quarter-pixel phase, where its red ink reaches a column right of its green ink.
#[test]
fn the_red_ink_of_subpixel_text_leads_the_blue_ink_by_about_two_thirds_of_a_pixel() {
    let lead = red_lead(&drawn("Wg", [10.0, 40.0]));

    assert!((0.4..0.9).contains(&lead), "red leads blue by {lead}");
}
