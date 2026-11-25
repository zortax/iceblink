//! Groups, filters and backdrops.

use std::sync::Arc;

use iceblink::{
    Backdrop, ClipEntry, ClipLinkEntry, ClipRef, Color, FRAME_NO_CLIP, Filter, FrameBackdrop,
    FrameClipShape, FrameFilter, FrameGroup, ROUND, SpaceRef, read_extent,
};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, filled, frosted, group, opaque};
use crate::conformance::draw::canvas::{Canvas, SIZE, golden};
use crate::conformance::draw::cases::cell;
use crate::golden::Tolerance;

/// Composited groups: a pixel may differ by a few steps where targets are blended or resampled.
const COMPOSITES: Tolerance = Tolerance {
    max_channel: 3,
    max_pixels: 32,
};

/// Blurred content: a pixel may differ by a few steps across a blur's falloff.
const BLURS: Tolerance = Tolerance {
    max_channel: 4,
    max_pixels: 128,
};

/// Inverts the colour and keeps the alpha.
const INVERT: [f32; 20] = [
    -1.0, 0.0, 0.0, 0.0, 1.0, //
    0.0, -1.0, 0.0, 0.0, 1.0, //
    0.0, 0.0, -1.0, 0.0, 1.0, //
    0.0, 0.0, 0.0, 1.0, 0.0,
];

/// Draws two overlapping squares from the corner of a cell, red under blue.
fn overlapping(builder: &mut FrameBuilder, [x, y, _, _]: [f32; 4]) {
    let red = builder.solid(opaque(220, 30, 30));
    let blue = builder.solid(opaque(30, 30, 220));
    builder.quad(filled([x, y, 24.0, 24.0], red));
    builder.quad(filled([x + 12.0, y + 12.0, 24.0, 24.0], blue));
}

/// Fills the output with white.
fn white(builder: &mut FrameBuilder) {
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], white));
}

/// Groups that composite once with their opacity, nest, cut their composite to a clip, and draw
/// nothing at opacity zero.
pub fn groups(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    white(&mut builder);

    builder.begin_group(group(cell(0), 0.5, &[]));
    overlapping(&mut builder, cell(0));
    builder.end_group();

    builder.begin_group(group(cell(1), 0.8, &[]));
    builder.begin_group(group(cell(1), 0.5, &[]));
    overlapping(&mut builder, cell(1));
    builder.end_group();
    builder.end_group();

    let [x, y, width, height] = cell(2);
    let clip = builder.clip(ClipEntry {
        links: [ClipLinkEntry {
            space: 0,
            shape: FrameClipShape::RoundedRect {
                rect: [x + 4.0, y + 4.0, width - 8.0, height - 8.0],
                radii: [8.0; 8],
                shapes: ROUND,
            },
        }]
        .into_iter()
        .collect(),
    });
    builder.begin_group(FrameGroup {
        clip: clip.0,
        ..group(cell(2), 0.9, &[])
    });
    overlapping(&mut builder, cell(2));
    builder.end_group();

    builder.begin_group(group(cell(3), 0.0, &[]));
    overlapping(&mut builder, cell(3));
    builder.end_group();

    golden("groups", &canvas.draw(builder.finish()), COMPOSITES);
}

/// Each filter alone and a chain of two.
pub fn filters(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    white(&mut builder);

    let grey: [f32; 20] = [
        0.3, 0.59, 0.11, 0.0, 0.0, //
        0.3, 0.59, 0.11, 0.0, 0.0, //
        0.3, 0.59, 0.11, 0.0, 0.0, //
        0.0, 0.0, 0.0, 1.0, 0.0,
    ];
    let chains = [
        vec![Filter::Blur(3.0)],
        vec![Filter::DropShadow {
            offset: [3.0, 4.0],
            std_dev: 2.0,
            color: Color([0.0, 0.0, 0.0, 0.7]),
        }],
        vec![Filter::ColorMatrix(INVERT)],
        vec![Filter::Opacity(0.4)],
        vec![Filter::ColorMatrix(grey), Filter::Blur(1.5)],
        vec![Filter::Blur(0.0)],
    ];
    for (index, chain) in chains.iter().enumerate() {
        let [x, y, _, _] = cell(index);
        builder.begin_group(group([x, y, 36.0, 36.0], 1.0, chain));
        overlapping(&mut builder, cell(index));
        builder.end_group();
    }
    golden("filters", &canvas.draw(builder.finish()), BLURS);
}

/// Backdrops that blur and invert what is beneath them, one of them inside a group.
pub fn backdrops(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    white(&mut builder);
    let black = builder.solid(opaque(0, 0, 0));
    for stripe in 0..16 {
        builder.quad(filled([stripe as f32 * 8.0, 0.0, 4.0, 128.0], black));
    }

    builder.backdrop(frosted([12.0, 12.0, 48.0, 40.0], 4.0));
    let bounds = [68.0, 12.0, 48.0, 40.0];
    builder.backdrop(FrameBackdrop {
        backdrop: Backdrop {
            bounds,
            filters: 0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
        },
        bounds,
        source: read_extent(bounds, &[Filter::ColorMatrix(INVERT)]),
        filters: Arc::from(vec![FrameFilter::ColorMatrix(INVERT)]),
    });

    builder.begin_group(group([8.0, 68.0, 112.0, 52.0], 0.9, &[]));
    let green = builder.solid(opaque(20, 160, 60));
    builder.quad(filled([8.0, 68.0, 56.0, 52.0], green));
    builder.backdrop(frosted([40.0, 76.0, 56.0, 36.0], 3.0));
    builder.end_group();

    golden("backdrops", &canvas.draw(builder.finish()), BLURS);
}
