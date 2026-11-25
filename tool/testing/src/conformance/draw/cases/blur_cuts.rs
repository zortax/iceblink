//! Blurred groups whose written region is cut where their content continues.

use iceblink::{ClipEntry, ClipLinkEntry, Filter, FrameClipShape, FrameGroup, ROUND};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, filled, group, opaque};
use crate::conformance::draw::canvas::{Canvas, SIDE, SIZE, golden};
use crate::conformance::draw::cases::gauss::phi;
use crate::golden::{RgbaImage, Tolerance};

/// The deviation of both blurs, in device pixels.
const DEVIATION: f32 = 4.0;

/// The colour the content is filled with.
const GREEN: [u8; 3] = [0, 153, 51];

/// The colour of the content that lies wholly outside the written region.
const RED: [u8; 3] = [204, 34, 0];

/// How far a pixel may be from the convolution of the content with a Gaussian, in eight-bit
/// steps.
// The blur runs at half resolution.
const FORMULA_STEPS: u8 = 8;

/// How far another renderer may differ from the golden.
const RENDERERS: Tolerance = Tolerance {
    max_channel: 2,
    max_pixels: 0,
};

/// The green content of the window case, past every side of the output.
const WINDOW_GREEN: [f32; 4] = [-40.0, -40.0, 208.0, 208.0];

/// The red content of the window case, wholly past the output's right edge.
const WINDOW_RED: [f32; 4] = [130.0, 40.0, 20.0, 48.0];

/// The clip's box in the clip case.
const CLIP: [f32; 4] = [32.0, 32.0, 64.0, 64.0];

/// The green content of the clip case, past every side of the clip.
const CLIP_GREEN: [f32; 4] = [16.0, 16.0, 96.0, 96.0];

/// The red content of the clip case, wholly past the clip's right edge.
const CLIP_RED: [f32; 4] = [100.0, 48.0, 12.0, 32.0];

/// A blurred group that fills the window, over content that continues past every edge of the
/// output, with a red patch past the right edge. The edges show the blurred content without a
/// fade, and the red patch bleeds in across the right edge.
pub fn blur_window(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    let side = SIDE as f32;
    let blur = [Filter::Blur(DEVIATION)];
    let content = union(WINDOW_GREEN, WINDOW_RED);
    builder.begin_group(FrameGroup {
        bounds: [0.0, 0.0, side, side],
        ..group(content, 1.0, &blur)
    });
    content_of(&mut builder, WINDOW_GREEN, WINDOW_RED);
    builder.end_group();

    let image = canvas.draw(builder.finish());
    for (x, y) in [(0, 0), (127, 0), (0, 127), (127, 127), (64, 0), (0, 64)] {
        assert_blurred(&image, [x, y], WINDOW_RED);
    }
    for x in 110..SIDE {
        assert_blurred(&image, [x, 64], WINDOW_RED);
    }
    golden("blur_window", &image, RENDERERS);
}

/// A blurred group cut to a clip's box, over content that continues past every edge of the box,
/// with a red patch past its right edge. Inside the box the blurred content shows without a fade,
/// and the red patch bleeds in across the box's right edge.
pub fn blur_clip(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, SIDE as f32, SIDE as f32], white));
    let clip = builder.clip(ClipEntry {
        links: [ClipLinkEntry {
            space: 0,
            shape: FrameClipShape::RoundedRect {
                rect: CLIP,
                radii: [0.0; 8],
                shapes: ROUND,
            },
        }]
        .into_iter()
        .collect(),
    });
    let blur = [Filter::Blur(DEVIATION)];
    let content = union(CLIP_GREEN, CLIP_RED);
    builder.begin_group(FrameGroup {
        bounds: CLIP,
        clip: clip.0,
        ..group(content, 1.0, &blur)
    });
    content_of(&mut builder, CLIP_GREEN, CLIP_RED);
    builder.end_group();

    let image = canvas.draw(builder.finish());
    for (x, y) in [(32, 32), (95, 32), (32, 95), (95, 95), (64, 32), (32, 64)] {
        assert_blurred(&image, [x, y], CLIP_RED);
    }
    for x in 80..96 {
        assert_blurred(&image, [x, 64], CLIP_RED);
    }
    golden("blur_clip", &image, RENDERERS);
}

/// Draws the green content and the red patch over it.
fn content_of(builder: &mut FrameBuilder, green: [f32; 4], red: [f32; 4]) {
    let green_paint = builder.solid(opaque(GREEN[0], GREEN[1], GREEN[2]));
    let red_paint = builder.solid(opaque(RED[0], RED[1], RED[2]));
    builder.quad(filled(green, green_paint));
    builder.quad(filled(red, red_paint));
}

fn union(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let left = a[0].min(b[0]);
    let top = a[1].min(b[1]);
    let right = (a[0] + a[2]).max(b[0] + b[2]);
    let bottom = (a[1] + a[3]).max(b[1] + b[3]);
    [left, top, right - left, bottom - top]
}

/// Asserts that `pixel` shows the opaque green content blurred, with `red` drawn over it.
#[track_caller]
fn assert_blurred(image: &RgbaImage, pixel: [u32; 2], red: [f32; 4]) {
    let [x, y] = pixel.map(|at| at as f32 + 0.5);
    let [left, top, width, height] = red;
    let spread =
        |at: f32, start: f32, end: f32| phi((at - start) / DEVIATION) - phi((at - end) / DEVIATION);
    let share = spread(x, left, left + width) * spread(y, top, top + height);
    let found = image.pixel(pixel[0], pixel[1]);
    for channel in 0..3 {
        let green = f32::from(GREEN[channel]);
        let expected = (green + (f32::from(RED[channel]) - green) * share).round() as u8;
        assert!(
            found[channel].abs_diff(expected) <= FORMULA_STEPS,
            "pixel {pixel:?} is {found:?}, where the blurred content is {expected} in channel \
             {channel}"
        );
    }
}
