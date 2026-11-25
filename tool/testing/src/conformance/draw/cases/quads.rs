//! Quads: corner radii and exponents, borders and their dash patterns.

use iceblink::geom::Affine2;
use iceblink::{BorderStyle, Color, FramePaint, Matrix4, Quad, SpaceRef};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, filled, opaque, straight};
use crate::conformance::draw::canvas::{Canvas, SIZE, golden};
use crate::conformance::draw::cases::cell;
use crate::golden::Tolerance;

/// Antialiased edges and corners: a pixel may differ by a few steps where its coverage is
/// partial.
const EDGES: Tolerance = Tolerance {
    max_channel: 3,
    max_pixels: 24,
};

/// Returns a quad over `bounds` with `radii` and `shapes`, filled with a new blue paint.
fn rounded(
    builder: &mut FrameBuilder,
    bounds: [f32; 4],
    radii: [f32; 8],
    shapes: [f32; 4],
) -> Quad {
    let fill = builder.solid(opaque(40, 90, 200));
    Quad {
        radii,
        shapes,
        ..filled(bounds, fill)
    }
}

/// Rounded, elliptical and superellipse corners, a translucent fill, and a turned and a scaled
/// quad.
pub fn corners(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], white));

    let shapes = [
        ([10.0; 8], [2.0; 4]),
        ([16.0, 6.0, 16.0, 6.0, 16.0, 6.0, 16.0, 6.0], [2.0; 4]),
        ([14.0; 8], [4.0; 4]),
        ([12.0; 8], [1.0; 4]),
        ([12.0; 8], [0.5; 4]),
        (
            [2.0, 2.0, 18.0, 18.0, 6.0, 12.0, 0.0, 0.0],
            [2.0, 3.0, 2.0, 2.0],
        ),
    ];
    for (index, (radii, exponents)) in shapes.into_iter().enumerate() {
        let quad = rounded(&mut builder, cell(index), radii, exponents);
        builder.quad(quad);
    }

    let translucent = builder.solid(straight(200, 30, 30, 128));
    builder.quad(Quad {
        radii: [8.0; 8],
        ..filled(cell(6), translucent)
    });
    let turn = Matrix4::translation(-18.0, -18.0, 0.0)
        .then(&Affine2::rotation(0.5).to_matrix4())
        .then(&Matrix4::translation(64.0, 106.0, 0.0));
    let turned = builder.space(turn);
    let mut quad = rounded(&mut builder, [0.0, 0.0, 36.0, 36.0], [9.0; 8], [2.0; 4]);
    quad.space = SpaceRef(turned);
    builder.quad(quad);

    let scaled = builder.space(Matrix4::scale(2.0, 2.0, 1.0));
    let mut quad = rounded(
        &mut builder,
        [44.0, 46.0, 18.0, 18.0],
        [5.0, 3.0, 5.0, 3.0, 5.0, 3.0, 5.0, 3.0],
        [2.0; 4],
    );
    quad.space = SpaceRef(scaled);
    builder.quad(quad);

    golden("corners", &canvas.draw(builder.finish()), EDGES);
}

/// Solid, dashed and dotted borders with and without radii, uneven widths and per-side colours.
pub fn borders(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], white));
    let fill = builder.solid(opaque(250, 220, 120));
    let stroke = builder.solid(opaque(30, 30, 30));
    let sides = builder.paint(FramePaint::Sides([
        Color([1.0, 0.0, 0.0, 1.0]),
        Color([0.0, 0.6, 0.0, 1.0]),
        Color([0.0, 0.0, 1.0, 1.0]),
        Color([0.5, 0.0, 0.5, 1.0]),
    ]));

    let bordered = |bounds, radii, border, style, stroke| Quad {
        radii: [radii; 8],
        border,
        stroke,
        style,
        ..filled(bounds, fill)
    };
    let quads = [
        bordered(cell(0), 8.0, [4.0; 4], BorderStyle::SOLID, stroke),
        bordered(
            cell(1),
            10.0,
            [2.0, 6.0, 4.0, 8.0],
            BorderStyle::SOLID,
            stroke,
        ),
        bordered(cell(2), 0.0, [3.0; 4], BorderStyle::DASHED, stroke),
        bordered(cell(3), 0.0, [4.0; 4], BorderStyle::DOTTED, stroke),
        bordered(cell(4), 12.0, [3.0; 4], BorderStyle::DASHED, stroke),
        bordered(cell(5), 12.0, [4.0; 4], BorderStyle::DOTTED, stroke),
        bordered(
            cell(6),
            0.0,
            [5.0, 3.0, 5.0, 3.0],
            BorderStyle::SOLID,
            sides,
        ),
        bordered(cell(7), 10.0, [6.0; 4], BorderStyle::SOLID, sides),
        bordered(
            cell(8),
            18.0,
            [0.0, 6.0, 0.0, 6.0],
            BorderStyle::SOLID,
            stroke,
        ),
    ];
    for quad in quads {
        builder.quad(quad);
    }
    golden("borders", &canvas.draw(builder.finish()), EDGES);
}
