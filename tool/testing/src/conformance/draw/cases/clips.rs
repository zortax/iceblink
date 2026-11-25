//! Clip chains: hard edges, rounded, elliptical and superellipse links, links in scaled and
//! turned spaces, chains of two, and alpha and luminance masks.

use iceblink::geom::Affine2;
use iceblink::{
    AtlasTile, ClipEntry, ClipLinkEntry, FrameClipShape, MaskMode, Matrix4, PageFormat, ROUND,
};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, filled, opaque};
use crate::conformance::draw::canvas::{Canvas, SIZE, golden, page};
use crate::conformance::draw::cases::cell;
use crate::golden::Tolerance;

/// Antialiased clip edges: a pixel may differ by a few steps where its coverage is partial.
const EDGES: Tolerance = Tolerance {
    max_channel: 3,
    max_pixels: 32,
};

fn rounded(space: u32, rect: [f32; 4], radii: [f32; 8], shapes: [f32; 4]) -> ClipLinkEntry {
    ClipLinkEntry {
        space,
        shape: FrameClipShape::RoundedRect {
            rect,
            radii,
            shapes,
        },
    }
}

fn inset([x, y, width, height]: [f32; 4], by: f32) -> [f32; 4] {
    [x + by, y + by, width - 2.0 * by, height - 2.0 * by]
}

/// A mask of eight by eight texels: alpha that ramps across, and luminance that ramps down.
fn mask() -> Vec<u8> {
    let mut bytes = Vec::new();
    for y in 0..8u32 {
        for x in 0..16u32 {
            if x < 8 {
                let alpha = (x * 255 / 7) as u8;
                bytes.extend_from_slice(&[0, 0, 0, alpha]);
            } else {
                let level = (y * 255 / 7) as u8;
                bytes.extend_from_slice(&[level, level, level, 255]);
            }
        }
    }
    bytes
}

/// Quads drawn through one chain each.
pub fn clips(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    page(
        &mut builder,
        2,
        PageFormat::RGBA8_PREMULTIPLIED,
        [16, 8],
        mask(),
    );
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], white));
    let scaled = builder.space(Matrix4::scale(2.0, 2.0, 1.0));
    let turn = Matrix4::translation(-18.0, -18.0, 0.0)
        .then(&Affine2::rotation(0.4).to_matrix4())
        .then(&Matrix4::translation(106.0, 64.0, 0.0));
    let turned = builder.space(turn);

    let chains: Vec<Vec<ClipLinkEntry>> = vec![
        vec![rounded(0, inset(cell(0), 4.5), [0.0; 8], ROUND)],
        vec![rounded(0, inset(cell(1), 2.0), [10.0; 8], ROUND)],
        vec![rounded(
            0,
            inset(cell(2), 2.0),
            [14.0, 5.0, 0.25, 3.0, 14.0, 5.0, 0.25, 3.0],
            ROUND,
        )],
        vec![rounded(0, inset(cell(3), 2.0), [14.0; 8], [4.0; 4])],
        {
            let [x, y, width, height] = inset(cell(4), 3.0);
            vec![rounded(
                scaled,
                [x / 2.0, y / 2.0, width / 2.0, height / 2.0],
                [4.0; 8],
                ROUND,
            )]
        },
        vec![rounded(turned, [4.0, 4.0, 28.0, 28.0], [0.0; 8], ROUND)],
        vec![
            rounded(0, inset(cell(6), 1.0), [16.0; 8], ROUND),
            rounded(
                0,
                [cell(6)[0] + 14.0, cell(6)[1], 36.0, 36.0],
                [6.0; 8],
                ROUND,
            ),
        ],
        vec![ClipLinkEntry {
            space: 0,
            shape: FrameClipShape::Mask {
                rect: cell(7),
                tile: AtlasTile {
                    page: 2,
                    rect: [0, 0, 8, 8],
                },
                mode: MaskMode::ALPHA,
            },
        }],
        vec![ClipLinkEntry {
            space: 0,
            shape: FrameClipShape::Mask {
                rect: cell(8),
                tile: AtlasTile {
                    page: 2,
                    rect: [8, 0, 8, 8],
                },
                mode: MaskMode::LUMINANCE,
            },
        }],
    ];
    let red = builder.solid(opaque(200, 30, 30));
    for (index, links) in chains.into_iter().enumerate() {
        let clip = builder.clip(ClipEntry {
            links: links.into_iter().collect(),
        });
        let mut quad = filled(cell(index), red);
        quad.clip = clip;
        builder.quad(quad);
    }
    golden("clips", &canvas.draw(builder.finish()), EDGES);
}

/// A rounded link covers nothing outside its rectangle grown by half a local unit.
///
/// The link has corners of radii 0.25 by 3 pixels, where the rounded-rectangle distance
/// underestimates, and is drawn in device space and in a space scaled by 2.
pub fn clip_link_boxes(make: MakeTarget<'_>) {
    for scale in [1.0, 2.0] {
        let Some(mut canvas) = Canvas::open(make) else {
            return;
        };
        let mut builder = FrameBuilder::new(SIZE);
        let space = builder.space(Matrix4::scale(scale, scale, 1.0));
        let white = builder.solid(opaque(255, 255, 255));
        let [x, y, width, height] = [5.25 / scale, 3.25 / scale, 56.0 / scale, 104.5 / scale];
        let [rx, ry] = [0.25 / scale, 3.0 / scale];
        let clip = builder.clip(ClipEntry {
            links: [rounded(
                space,
                [x, y, width, height],
                [rx, ry, rx, ry, 0.0, 0.0, 0.0, 0.0],
                ROUND,
            )]
            .into_iter()
            .collect(),
        });
        let mut quad = filled([0.0, 0.0, 128.0, 128.0], white);
        quad.clip = clip;
        builder.quad(quad);
        let image = canvas.draw(builder.finish());

        let mut lit = Vec::new();
        for row in 0..image.height() {
            for column in 0..image.width() {
                let px = (column as f32 + 0.5) / scale;
                let py = (row as f32 + 0.5) / scale;
                let outside =
                    px < x - 0.5 || py < y - 0.5 || px > x + width + 0.5 || py > y + height + 0.5;
                let value = image.pixel(column, row)[0];
                if outside && value != 0 {
                    lit.push((column, row, value));
                }
            }
        }
        assert!(
            lit.is_empty(),
            "under a scale of {scale}, pixels outside the link's box are lit: {lit:?}"
        );
    }
}
