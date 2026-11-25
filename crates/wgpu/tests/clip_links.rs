//! Clip chains: kept spaces that move without touching the clip table, deep chains, links under a
//! turn, and mask links.

mod common;

use std::sync::Arc;

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{counter, plain_renderer, present};
use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    AtlasTile, ClipEntry, ClipLinkEntry, FrameClipShape, MaskMode, Matrix4, PageFormat, PageOp,
    ROUND,
};
use iceblink_wgpu::WgpuRenderer;

fn rounded(space: u32, bounds: [f32; 4], radius: f32) -> ClipLinkEntry {
    ClipLinkEntry {
        space,
        shape: FrameClipShape::RoundedRect {
            rect: bounds,
            radii: [radius; 8],
            shapes: ROUND,
        },
    }
}

/// Returns how many table slots `renderer` has encoded.
fn prepared(renderer: &WgpuRenderer) -> u64 {
    counter(renderer, "table_slots_prepared")
}

#[test]
fn a_kept_space_that_scrolls_moves_its_clip_without_touching_the_clip_table() {
    let mut renderer = plain_renderer();
    let mut first = FrameBuilder::new();
    let scrolled = first.space(Matrix4::translation(0.0, 0.0, 0.0));
    let green = first.solid(opaque(0, 255, 0));
    let clip = first.clip(ClipEntry {
        links: [rounded(scrolled, rect(16.0, 16.0, 32.0, 32.0), 4.0)]
            .into_iter()
            .collect(),
    });
    let mut quad = filled(rect(0.0, 0.0, 64.0, 64.0), green);
    quad.space = iceblink::SpaceRef(scrolled);
    quad.clip = clip;
    first.quad(quad);

    let before = present(&mut renderer, &first.finish());

    assert_eq!(before.rgba(30, 30), [0, 255, 0, 255]);
    assert_eq!(before.rgba(60, 60), [0, 0, 0, 0]);
    let slots = prepared(&renderer);

    let mut next = FrameBuilder::next();
    next.set_space(scrolled, Matrix4::translation(40.0, 40.0, 0.0));
    next.quad(quad);

    let after = present(&mut renderer, &next.finish());

    assert_eq!(
        prepared(&renderer) - slots,
        1,
        "only the space slot was encoded again"
    );
    assert_eq!(
        after.rgba(70, 70),
        [0, 255, 0, 255],
        "the clip moved with its space"
    );
    assert_eq!(after.rgba(30, 30), [0, 0, 0, 0]);
}

#[test]
fn every_link_of_a_deep_chain_is_applied() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let green = builder.solid(opaque(0, 255, 0));
    let clip = builder.clip(ClipEntry {
        links: [
            rounded(0, rect(0.0, 0.0, 100.0, 100.0), 4.0),
            rounded(0, rect(10.0, 0.0, 100.0, 100.0), 4.0),
            rounded(0, rect(0.0, 20.0, 100.0, 100.0), 4.0),
        ]
        .into_iter()
        .collect(),
    });
    let mut quad = filled(rect(0.0, 0.0, 128.0, 128.0), green);
    quad.clip = clip;
    builder.quad(quad);

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(pixels.rgba(50, 50), [0, 255, 0, 255]);
    assert_eq!(
        pixels.rgba(5, 50),
        [0, 0, 0, 0],
        "the second link cuts the left"
    );
    assert_eq!(
        pixels.rgba(50, 10),
        [0, 0, 0, 0],
        "the third link cuts the top"
    );
}

#[test]
fn a_link_in_a_turned_space_is_evaluated_in_its_own_space() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let turn = Matrix4::translation(-64.0, -64.0, 0.0)
        .then(&iceblink::geom::Affine2::rotation(core::f32::consts::FRAC_PI_4).to_matrix4())
        .then(&Matrix4::translation(64.0, 64.0, 0.0));
    let turned = builder.space(turn);
    let green = builder.solid(opaque(0, 255, 0));
    let clip = builder.clip(ClipEntry {
        links: [rounded(turned, rect(44.0, 44.0, 40.0, 40.0), 0.0)]
            .into_iter()
            .collect(),
    });
    let mut quad = filled(rect(0.0, 0.0, 128.0, 128.0), green);
    quad.clip = clip;
    builder.quad(quad);

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(pixels.rgba(64, 64), [0, 255, 0, 255], "the centre");
    assert_eq!(
        pixels.rgba(64, 40),
        [0, 255, 0, 255],
        "a corner of the turned square"
    );
    assert_eq!(
        pixels.rgba(46, 46),
        [0, 0, 0, 0],
        "a corner of the unturned square"
    );
}

#[test]
fn a_square_link_under_a_quarter_turn_is_a_hard_edge() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    // A quarter turn about the point (64, 64): x' = 128 - y and y' = x.
    let turn = Matrix4::from_columns([
        [0.0, 1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [128.0, 0.0, 0.0, 1.0],
    ]);
    let turned = builder.space(turn);
    let green = builder.solid(opaque(0, 255, 0));
    // On the device the rectangle spans 58 to 98 across and 20.5 to 60.5 down, so its top and
    // bottom edges pass through pixel centres.
    let clip = builder.clip(ClipEntry {
        links: [rounded(turned, rect(20.5, 30.0, 40.0, 40.0), 0.0)]
            .into_iter()
            .collect(),
    });
    let mut quad = filled(rect(0.0, 0.0, 128.0, 128.0), green);
    quad.clip = clip;
    builder.quad(quad);

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(pixels.rgba(70, 20), [0, 255, 0, 255], "the top edge");
    assert_eq!(pixels.rgba(70, 60), [0, 255, 0, 255], "the bottom edge");
    assert_eq!(pixels.rgba(70, 61), [0, 0, 0, 0], "below the bottom edge");
}

#[test]
fn a_chain_of_more_than_255_general_links_applies_every_link() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let turn = Matrix4::translation(-64.0, -64.0, 0.0)
        .then(&iceblink::geom::Affine2::rotation(core::f32::consts::FRAC_PI_4).to_matrix4())
        .then(&Matrix4::translation(64.0, 64.0, 0.0));
    let turned = builder.space(turn);
    let green = builder.solid(opaque(0, 255, 0));
    // Links in a turned space are general links. The first 300 cover the output, and the last
    // only a diamond around its centre.
    let mut links: Vec<ClipLinkEntry> = (0..300)
        .map(|_| rounded(turned, rect(-500.0, -500.0, 1128.0, 1128.0), 0.0))
        .collect();
    links.push(rounded(turned, rect(54.0, 54.0, 20.0, 20.0), 0.0));
    let clip = builder.clip(ClipEntry {
        links: links.into_iter().collect(),
    });
    let mut quad = filled(rect(0.0, 0.0, 128.0, 128.0), green);
    quad.clip = clip;
    builder.quad(quad);

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(pixels.rgba(64, 64), [0, 255, 0, 255], "the centre");
    assert_eq!(
        pixels.rgba(100, 30),
        [0, 0, 0, 0],
        "the last link cuts everything outside the diamond"
    );
}

#[test]
fn a_mask_link_samples_its_tile_and_follows_a_write_to_its_page() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    builder.page(PageOp::Create {
        page: 0,
        format: PageFormat::A8,
        size: [16, 16],
        levels: 1,
    });
    // The left half of the tile covers, the right half does not.
    let mut bytes = vec![0u8; 16 * 16];
    for row in bytes.chunks_mut(16) {
        row[..8].fill(255);
    }
    builder.page(PageOp::Upload {
        page: 0,
        level: 0,
        rect: Rect::new(Point::new(0, 0), Size::new(16, 16)),
        bytes: Arc::from(bytes),
        offset: 0,
        stride: 16,
    });
    let green = builder.solid(opaque(0, 255, 0));
    let clip = builder.clip(ClipEntry {
        links: [ClipLinkEntry {
            space: 0,
            shape: FrameClipShape::Mask {
                rect: rect(0.0, 0.0, 128.0, 128.0),
                tile: AtlasTile {
                    page: 0,
                    rect: [0, 0, 16, 16],
                },
                mode: MaskMode::ALPHA,
            },
        }]
        .into_iter()
        .collect(),
    });
    let mut quad = filled(rect(0.0, 0.0, 128.0, 128.0), green);
    quad.clip = clip;
    builder.quad(quad);

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(pixels.rgba(16, 64), [0, 255, 0, 255]);
    assert_eq!(pixels.rgba(112, 64), [0, 0, 0, 0]);

    let mut next = FrameBuilder::next();
    next.page(PageOp::Upload {
        page: 0,
        level: 0,
        rect: Rect::new(Point::new(0, 0), Size::new(16, 16)),
        bytes: Arc::from(vec![255u8; 256]),
        offset: 0,
        stride: 16,
    });
    next.quad(quad);

    let pixels = present(&mut renderer, &next.finish());

    assert_eq!(
        pixels.rgba(112, 64),
        [0, 255, 0, 255],
        "the rewritten tile covers"
    );
}

/// Adds a 16 by 16 page whose texel at `(x, y)` is `texel(x, y)`.
fn mask_page(
    builder: &mut FrameBuilder,
    page: u32,
    format: PageFormat,
    texel: impl Fn(usize, usize) -> Vec<u8>,
) {
    builder.page(PageOp::Create {
        page,
        format,
        size: [16, 16],
        levels: 1,
    });
    let bytes: Vec<u8> = (0..16 * 16)
        .flat_map(|at| texel(at % 16, at / 16))
        .collect();
    let stride = bytes.len() as u32 / 16;
    builder.page(PageOp::Upload {
        page,
        level: 0,
        rect: Rect::new(Point::new(0, 0), Size::new(16, 16)),
        bytes: Arc::from(bytes),
        offset: 0,
        stride,
    });
}

/// Draws a white square over the whole output clipped by the links `chosen` picks out of a chain
/// of three rounded links and two masks, and returns the coverage of every pixel.
fn chain_coverage(chosen: &[usize]) -> Vec<u8> {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    // Alpha: the left twelve columns cover. Luminance: white rows, then grey, then black.
    mask_page(&mut builder, 0, PageFormat::A8, |x, _| {
        vec![if x < 12 { 255 } else { 0 }]
    });
    mask_page(&mut builder, 1, PageFormat::RGBA8_PREMULTIPLIED, |_, y| {
        let value = match y {
            0..8 => 255,
            8..12 => 128,
            _ => 0,
        };
        vec![value, value, value, 255]
    });
    let doubled = builder.space(Matrix4::scale(2.0, 2.0, 1.0));
    let mask = |page: u32, mode: MaskMode| ClipLinkEntry {
        space: 0,
        shape: FrameClipShape::Mask {
            rect: rect(0.0, 0.0, 128.0, 128.0),
            tile: AtlasTile {
                page,
                rect: [0, 0, 16, 16],
            },
            mode,
        },
    };
    let chain = [
        rounded(0, rect(4.0, 4.0, 120.0, 120.0), 30.0),
        mask(0, MaskMode::ALPHA),
        rounded(0, rect(10.0, 0.0, 118.0, 128.0), 12.0),
        rounded(doubled, rect(4.0, 6.0, 56.0, 52.0), 8.0),
        mask(1, MaskMode::LUMINANCE),
    ];
    let clip = builder.clip(ClipEntry {
        links: chosen.iter().map(|index| chain[*index].clone()).collect(),
    });
    let white = builder.solid(opaque(255, 255, 255));
    let mut quad = filled(rect(0.0, 0.0, 128.0, 128.0), white);
    quad.clip = clip;
    builder.quad(quad);
    let pixels = present(&mut renderer, &builder.finish());
    let side = pixels.size().width;
    (0..side * side)
        .map(|at| pixels.rgba(at % side, at / side)[1])
        .collect()
}

#[test]
fn a_chain_of_three_rounded_links_and_two_masks_covers_the_product_of_its_links() {
    let links = [0, 1, 2, 3, 4];
    let whole = chain_coverage(&links);
    let alone: Vec<Vec<u8>> = links.iter().map(|link| chain_coverage(&[*link])).collect();

    let mut worst = 0u8;
    for (at, covered) in whole.iter().enumerate() {
        let product = alone
            .iter()
            .map(|coverage| f32::from(coverage[at]) / 255.0)
            .product::<f32>();
        let expected = (product * 255.0).round() as u8;
        worst = worst.max(covered.abs_diff(expected));
    }
    assert!(
        worst <= 3,
        "every pixel is the product of the links, off by {worst}"
    );
    let at = |x: usize, y: usize| whole[y * 128 + x];
    assert_eq!(at(60, 30), 255, "inside every link");
    assert_eq!(at(100, 30), 0, "the alpha mask cuts the right");
    assert_eq!(at(60, 110), 0, "the luminance mask cuts the bottom");
    assert!(
        (110..=146).contains(&at(60, 84)),
        "the grey rows halve the coverage: {}",
        at(60, 84)
    );
    assert_eq!(at(5, 60), 0, "the second link cuts the left");
    assert_eq!(at(60, 4), 0, "the scaled link cuts the top");
}
