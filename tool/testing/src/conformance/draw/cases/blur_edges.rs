//! Blurred groups whose content reaches the edge of their bounds.

use iceblink::{Color, Filter, FrameGroup};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, filled, group, opaque};
use crate::conformance::draw::canvas::{Canvas, SIDE, SIZE, golden};
use crate::conformance::draw::cases::gauss::phi;
use crate::golden::{RgbaImage, Tolerance};

/// The deviation of both blurs, in device pixels.
const DEVIATION: f32 = 4.0;

/// The content's colour.
const GREEN: [u8; 3] = [0, 153, 51];

/// The horizontal offset of the shadow of [`SHADOWED`], which falls to the left.
const SHADOW_OFFSET: f32 = -8.0;

/// The deviation of the shadow of [`SHADOWED`].
const SHADOW_DEVIATION: f32 = 3.0;

/// The opacity of both shadows, which are black.
const SHADOW_OPACITY: f32 = 0.8;

/// How far the shadow of [`OFFSET`] falls to the right.
const FAR_OFFSET: f32 = 6.0;

/// The deviation of the shadow of [`OFFSET`].
const FAR_DEVIATION: f32 = 2.0;

/// How far a pixel may be from the convolution of the content with a Gaussian, in eight-bit
/// steps.
// A blur at half resolution draws the pixel on the edge of the bounds from the texel it shares
// with its neighbour, 13 steps from the formula.
const FORMULA_STEPS: u8 = 16;

/// How far another renderer may differ from the golden.
const RENDERERS: Tolerance = Tolerance {
    max_channel: 2,
    max_pixels: 0,
};

/// The square at the output's right edge.
const AT_OUTPUT: [f32; 4] = [88.0, 24.0, 40.0, 40.0];

/// The square that fills its group's bounds.
const AT_BOUNDS: [f32; 4] = [16.0, 24.0, 40.0, 40.0];

/// The square whose shadow falls to its left, in a group that ends at its top and bottom.
const SHADOWED: [f32; 4] = [28.0, 84.0, 32.0, 32.0];

/// The bounds of the shadowed square's group.
const SHADOWED_BOUNDS: [f32; 4] = [16.0, 84.0, 44.0, 32.0];

/// The square whose shadow falls further right than it blurs, in a group over its read extent.
const OFFSET: [f32; 4] = [80.0, 88.0, 24.0, 24.0];

/// Two blurred squares and two shadowed ones. The first is in a group cut by the output's right
/// edge, the second in a group whose bounds are the square itself, and the third in a group that
/// ends at the square's top and bottom. Content outside a group's bounds is transparent, so each
/// blur fades towards the edge of the bounds by the Gaussian's integral. The fourth casts its
/// shadow further than the shadow's blur reaches, so nothing is drawn left of it.
pub fn blur_edges(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, SIDE as f32, SIDE as f32], white));
    let green = builder.solid(opaque(GREEN[0], GREEN[1], GREEN[2]));

    let blur = [Filter::Blur(DEVIATION)];
    let mut at_output = group(AT_OUTPUT, 1.0, &blur);
    at_output.bounds = cut_to_output(at_output.bounds);
    builder.begin_group(at_output.clone());
    builder.quad(filled(AT_OUTPUT, green));
    builder.end_group();

    builder.begin_group(FrameGroup {
        bounds: AT_BOUNDS,
        ..group(AT_BOUNDS, 1.0, &blur)
    });
    builder.quad(filled(AT_BOUNDS, green));
    builder.end_group();

    let shadow = [Filter::DropShadow {
        offset: [SHADOW_OFFSET, 0.0],
        std_dev: SHADOW_DEVIATION,
        color: Color([0.0, 0.0, 0.0, SHADOW_OPACITY]),
    }];
    builder.begin_group(FrameGroup {
        bounds: SHADOWED_BOUNDS,
        ..group(SHADOWED, 1.0, &shadow)
    });
    builder.quad(filled(SHADOWED, green));
    builder.end_group();

    let far = [Filter::DropShadow {
        offset: [FAR_OFFSET, 0.0],
        std_dev: FAR_DEVIATION,
        color: Color([0.0, 0.0, 0.0, SHADOW_OPACITY]),
    }];
    builder.begin_group(group(OFFSET, 1.0, &far));
    builder.quad(filled(OFFSET, green));
    builder.end_group();

    let image = canvas.draw(builder.finish());
    let centre = |[x, y, width, height]: [f32; 4]| [x + width / 2.0, y + height / 2.0];
    let [_, output_row] = centre(AT_OUTPUT);
    let [bounds_column, bounds_row] = centre(AT_BOUNDS);
    let green = GREEN.map(f32::from);
    for x in 112..SIDE {
        let alpha = blurred(
            [x, output_row as u32],
            AT_OUTPUT,
            at_output.bounds,
            DEVIATION,
        );
        assert_over_white(&image, [x, output_row as u32], green, alpha);
    }
    for x in 16..32 {
        let alpha = blurred([x, bounds_row as u32], AT_BOUNDS, AT_BOUNDS, DEVIATION);
        assert_over_white(&image, [x, bounds_row as u32], green, alpha);
    }
    for y in 24..40 {
        let alpha = blurred([bounds_column as u32, y], AT_BOUNDS, AT_BOUNDS, DEVIATION);
        assert_over_white(&image, [bounds_column as u32, y], green, alpha);
    }
    // The shadow is the square's alpha blurred and moved left, so it shows left of the square.
    let [left, top, width, height] = SHADOWED;
    let cast = [left + SHADOW_OFFSET, top, width, height];
    let cast_bounds = [
        SHADOWED_BOUNDS[0] + SHADOW_OFFSET,
        SHADOWED_BOUNDS[1],
        SHADOWED_BOUNDS[2],
        SHADOWED_BOUNDS[3],
    ];
    for y in 84..100 {
        let alpha = blurred([24, y], cast, cast_bounds, SHADOW_DEVIATION) * SHADOW_OPACITY;
        assert_over_white(&image, [24, y], [0.0; 3], alpha);
    }
    let [far_left, far_top, _, far_height] = OFFSET;
    let row = (far_top + far_height / 2.0) as u32;
    for x in far_left as u32 - 2..far_left as u32 {
        let cast = [x - FAR_OFFSET as u32, row];
        let alpha = blurred(cast, OFFSET, OFFSET, FAR_DEVIATION) * SHADOW_OPACITY;
        assert_over_white(&image, [x, row], [0.0; 3], alpha);
    }
    golden("blur_edges", &image, RENDERERS);
}

fn cut_to_output([x, y, width, height]: [f32; 4]) -> [f32; 4] {
    let side = SIDE as f32;
    let left = x.max(0.0);
    let top = y.max(0.0);
    [
        left,
        top,
        (x + width).min(side) - left,
        (y + height).min(side) - top,
    ]
}

/// Returns the alpha at `pixel` of an opaque `square` blurred by `deviation`, with nothing outside
/// `bounds`.
fn blurred(pixel: [u32; 2], square: [f32; 4], bounds: [f32; 4], deviation: f32) -> f32 {
    let [x, y] = pixel.map(|at| at as f32 + 0.5);
    let [left, top, width, height] = square;
    let [bounds_left, bounds_top, bounds_width, bounds_height] = bounds;
    let spread = |at: f32, start: f32, end: f32, from: f32, to: f32| {
        let start = start.max(from);
        let end = end.min(to);
        phi((at - start) / deviation) - phi((at - end) / deviation)
    };
    spread(
        x,
        left,
        left + width,
        bounds_left,
        bounds_left + bounds_width,
    ) * spread(y, top, top + height, bounds_top, bounds_top + bounds_height)
}

/// Asserts that `pixel` shows `color` at `alpha` over white, within `FORMULA_STEPS`.
#[track_caller]
fn assert_over_white(image: &RgbaImage, pixel: [u32; 2], color: [f32; 3], alpha: f32) {
    let found = image.pixel(pixel[0], pixel[1]);
    for channel in 0..3 {
        let expected = (color[channel] * alpha + 255.0 * (1.0 - alpha)).round() as u8;
        assert!(
            found[channel].abs_diff(expected) <= FORMULA_STEPS,
            "pixel {pixel:?} is {found:?}, where the blur with nothing outside its group is \
             {expected} in channel {channel}"
        );
    }
}
