//! Atlas pages: their upload staging, and how the texels of a tile reach the output.

mod common;

use std::sync::Arc;

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{counter, draw, plain_renderer, present};
use iceblink::Renderer as _;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    AtlasTile, ColorSprite, FRAME_NO_CLIP, PageFormat, PageOp, PaintRef, ROUND, Sprite, SpriteFlags,
};

const TILE: u32 = 16;

/// Adds the creation of page `page` of `format` and `size`, and an upload of `bytes` over all of
/// it.
fn page(builder: &mut FrameBuilder, page: u32, format: PageFormat, size: u32, bytes: Vec<u8>) {
    let texel = bytes.len() as u32 / (size * size);
    builder.page(PageOp::Create {
        page,
        format,
        size: [size, size],
        levels: 1,
    });
    builder.page(PageOp::Upload {
        page,
        level: 0,
        rect: Rect::new(Point::new(0, 0), Size::new(size as i32, size as i32)),
        bytes: Arc::from(bytes),
        offset: 0,
        stride: size * texel,
    });
}

/// Returns `TILE` rows of `texel(x)` for every column.
fn rows(texel: impl Fn(u32) -> Vec<u8>) -> Vec<u8> {
    (0..TILE * TILE)
        .flat_map(|index| texel(index % TILE))
        .collect()
}

/// Returns the coverage of column `x` of a ramp that falls from full at the left to none.
fn falling(x: u32) -> u8 {
    255 - (255 * x / (TILE - 1)) as u8
}

/// Returns a colour sprite of the whole tile on page 1 at the output's top left.
fn colour_tile() -> ColorSprite {
    let bounds = rect(0.0, 0.0, TILE as f32, TILE as f32);
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
        flags: SpriteFlags::default(),
    }
}

/// Returns a coverage sprite of the tile on page 0 at the output's top left, filled with `paint`.
fn coverage_tile(paint: PaintRef) -> Sprite {
    Sprite {
        bounds: rect(0.0, 0.0, TILE as f32, TILE as f32),
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

/// Returns the middle row of the tile's pixels.
fn middle_row(pixels: &iceblink_wgpu::Pixels, width: u32) -> Vec<[u8; 4]> {
    (0..width as i32)
        .map(|x| pixels.rgba(x, TILE as i32 / 2))
        .collect()
}

#[test]
fn an_upload_of_a_large_page_keeps_none_of_its_staging() {
    let mut renderer = plain_renderer();
    for page_id in 0..2 {
        let mut builder = FrameBuilder::new();
        page(
            &mut builder,
            page_id,
            PageFormat::RGBA8_PREMULTIPLIED,
            1024,
            vec![9; 1024 * 1024 * 4],
        );

        draw(&mut renderer, &builder.finish());

        let staging = counter(&renderer, "upload_staging_bytes");
        assert!(
            staging < 1 << 20,
            "a four-megabyte transfer leaves {staging} bytes of staging behind"
        );
    }
}

#[test]
fn a_large_upload_is_staged_in_chunks_of_at_most_a_mebibyte() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    page(
        &mut builder,
        0,
        PageFormat::RGBA8_PREMULTIPLIED,
        1024,
        vec![9; 1024 * 1024 * 4],
    );

    draw(&mut renderer, &builder.finish());

    // The chunks of the frame stay allocated until the device is next polled.
    let Some(report) = renderer.gpu().device().generate_allocator_report() else {
        return;
    };
    for chunk in report
        .allocations
        .iter()
        .filter(|allocation| allocation.name == "iceblink.upload.chunk")
    {
        assert!(chunk.size <= 1 << 20, "a chunk of {} bytes", chunk.size);
    }
}

#[test]
fn a_page_staged_in_several_pieces_draws_its_last_rows() {
    let side = 1024;
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let bytes = (0..side)
        .flat_map(|y| {
            let texel = if y < side - TILE {
                [255, 0, 0, 255]
            } else {
                [0, 255, 0, 255]
            };
            texel.repeat(side as usize)
        })
        .collect();
    page(
        &mut builder,
        1,
        PageFormat::RGBA8_PREMULTIPLIED,
        side,
        bytes,
    );
    builder.color(ColorSprite {
        tile: AtlasTile {
            page: 1,
            rect: [0, side - TILE, TILE, TILE],
        },
        ..colour_tile()
    });

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(pixels.rgba(8, 8), [0, 255, 0, 255]);
}

/// Returns a red ramp from opaque at the left to transparent, premultiplied.
fn premultiplied_ramp() -> Vec<u8> {
    rows(|x| {
        let alpha = falling(x);
        vec![alpha, 0, 0, alpha]
    })
}

/// Returns a frame that draws the colour tile over mid grey, creating page 1 from `bytes` when
/// given.
fn over_grey(bytes: Option<Vec<u8>>) -> iceblink::Frame {
    let mut builder = FrameBuilder::new();
    if let Some(bytes) = bytes {
        page(
            &mut builder,
            1,
            PageFormat::RGBA8_PREMULTIPLIED,
            TILE,
            bytes,
        );
    }
    let grey = builder.solid(opaque(128, 128, 128));
    builder.quad(filled(rect(0.0, 0.0, 128.0, 128.0), grey));
    builder.color(colour_tile());
    builder.finish()
}

#[test]
fn releasing_idle_resources_drops_the_staging_and_keeps_the_page() {
    let mut renderer = plain_renderer();
    let first = middle_row(
        &present(&mut renderer, &over_grey(Some(premultiplied_ramp()))),
        TILE,
    );

    draw(&mut renderer, &over_grey(None));
    renderer.release_idle_resources();

    assert_eq!(
        counter(&renderer, "upload_staging_bytes"),
        0,
        "an idle renderer holds no upload staging"
    );

    let again = middle_row(&present(&mut renderer, &over_grey(None)), TILE);
    assert_eq!(again, first, "the page keeps the tile");
}

#[test]
fn a_premultiplied_tile_composites_its_soft_edge_without_blooming() {
    let mut renderer = plain_renderer();
    let row = middle_row(
        &present(&mut renderer, &over_grey(Some(premultiplied_ramp()))),
        TILE,
    );

    assert_eq!(row[0], [255, 0, 0, 255], "the opaque end is the tile's red");
    assert_eq!(
        row[TILE as usize - 1],
        [128, 128, 128, 255],
        "the transparent end is the grey"
    );
    let half = row[TILE as usize / 2];
    assert!(
        (185..=189).contains(&half[0]),
        "half coverage adds half its red: {half:?}"
    );
    assert!(
        (66..=70).contains(&half[1]),
        "the grey shows through in proportion: {half:?}"
    );
}

/// Draws the coverage page `bytes` over white, tinted `tint`.
fn coverage_over_white(bytes: Vec<u8>, tint: [u8; 3]) -> Vec<[u8; 4]> {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    page(&mut builder, 0, PageFormat::A8, TILE, bytes);
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled(rect(0.0, 0.0, 128.0, 128.0), white));
    let tint = builder.solid(opaque(tint[0], tint[1], tint[2]));
    builder.mono(coverage_tile(tint));
    middle_row(&present(&mut renderer, &builder.finish()), TILE)
}

#[test]
fn a_coverage_tile_is_tinted_by_its_paint_in_proportion_to_its_coverage() {
    let row = coverage_over_white(rows(|x| vec![falling(x)]), [0, 0, 255]);

    assert_eq!(row[0], [0, 0, 255, 255], "full coverage is the tint");
    assert_eq!(
        row[TILE as usize - 1],
        [255; 4],
        "no coverage leaves the white"
    );
    for pair in row.windows(2) {
        assert!(
            pair[0][2] <= pair[1][2] && pair[0][0] <= pair[1][0],
            "coverage falls off monotonically: {row:?}"
        );
    }
}

#[test]
fn per_channel_coverage_puts_a_fringe_on_a_stroke() {
    let mut renderer = plain_renderer();
    let subpixel = renderer.capabilities().subpixel_text;
    let mut builder = FrameBuilder::new();
    let ramp = rows(|x| {
        let base = (255 * x / (TILE - 1)) as u8;
        vec![base, base.saturating_add(40), base.saturating_add(80), 255]
    });
    page(&mut builder, 0, PageFormat::SUBPIXEL, TILE, ramp);
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled(rect(0.0, 0.0, 128.0, 128.0), white));
    let black = builder.solid(opaque(0, 0, 0));
    builder.subpixel(coverage_tile(black));

    let sample = present(&mut renderer, &builder.finish()).rgba(TILE as i32 / 2, TILE as i32 / 2);

    if subpixel {
        assert_ne!(
            sample[0], sample[2],
            "each channel takes its own coverage: {sample:?}"
        );
    } else {
        assert_eq!(
            sample, [255; 4],
            "without dual-source blending nothing is drawn"
        );
    }
}
