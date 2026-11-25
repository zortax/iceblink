//! Hand-built frames of every kind the GPU renderer draws, drawn by both renderers, which must
//! agree within a tolerance.

mod common;

use std::sync::Arc;

use iceblink::{
    Backdrop, BlendMode, BorderStyle, ClipEntry, ClipLinkEntry, ClipRef, Color, FRAME_NO_CLIP,
    Frame, FrameBackdrop, FrameClipShape, FrameFilter, FrameGradient, FrameGroup, FramePaint,
    FrameShadow, FrameStop, GradientShape, Line, LineStyle, Matrix4, Quad, Shadow, SpaceRef,
    StopEncoding,
};
use iceblink_testing::golden::Tolerance;
use smallvec::smallvec;

use common::build::{FrameBuilder, filled};
use common::gpu::{assert_close, gpu};
use common::on_cpu;

/// Two steps of a channel.
const CLOSE: Tolerance = Tolerance {
    max_channel: 2,
    max_pixels: 0,
};

fn compare(what: &str, frame: &Frame, tolerance: Tolerance) {
    let Some(mut gpu) = gpu() else {
        return;
    };
    let expected = gpu.drawn(frame);
    let drawn = on_cpu(frame);
    assert_close(what, &expected, &drawn, tolerance);
}

fn white(builder: &mut FrameBuilder) {
    let paint = builder.solid([1.0; 4]);
    builder.quad(filled([0.0, 0.0, 96.0, 96.0], paint));
}

fn rotation(degrees: f32, about: [f32; 2]) -> Matrix4 {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let turn = Matrix4::from_columns([
        [cos, sin, 0.0, 0.0],
        [-sin, cos, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);
    Matrix4::translation(-about[0], -about[1], 0.0)
        .then(&turn)
        .then(&Matrix4::translation(about[0], about[1], 0.0))
}

fn gradient(shape: GradientShape, encoding: StopEncoding, repeat: bool) -> FramePaint {
    let stops: Vec<FrameStop> = match encoding {
        StopEncoding::OKLAB => vec![
            FrameStop {
                color: [0.628, 0.225, 0.126, 1.0],
                offset: 0.0,
            },
            FrameStop {
                color: [0.452, -0.032, -0.312, 1.0],
                offset: 1.0,
            },
        ],
        _ => vec![
            FrameStop {
                color: [1.0, 0.0, 0.0, 1.0],
                offset: 0.0,
            },
            FrameStop {
                color: [0.0, 0.25, 0.5, 0.5],
                offset: 0.6,
            },
            FrameStop {
                color: [0.0, 0.0, 1.0, 1.0],
                offset: 1.0,
            },
        ],
    };
    FramePaint::Gradient(FrameGradient {
        shape,
        encoding,
        repeat,
        stops: Arc::from(stops),
    })
}

#[test]
fn rounded_and_superelliptic_quads_with_borders() {
    let mut builder = FrameBuilder::new();
    white(&mut builder);
    let fill = builder.solid([0.2, 0.4, 0.8, 1.0]);
    let stroke = builder.solid([0.5, 0.0, 0.0, 0.5]);
    let sides = builder.paint(FramePaint::Sides([
        Color([1.0, 0.0, 0.0, 1.0]),
        Color([0.0, 1.0, 0.0, 1.0]),
        Color([0.0, 0.0, 1.0, 1.0]),
        Color([0.0, 0.0, 0.0, 1.0]),
    ]));
    let bordered = |bounds, radii, shapes, border, style: BorderStyle, stroke| Quad {
        radii,
        shapes,
        border,
        stroke,
        style,
        ..filled(bounds, fill)
    };
    let ellipses = [10.0, 6.0, 4.0, 4.0, 12.0, 12.0, 0.0, 0.0];
    let wide = [10.0, 4.0, 10.0, 4.0, 10.0, 4.0, 10.0, 4.0];
    for quad in [
        bordered(
            [4.0, 4.0, 40.0, 26.0],
            ellipses,
            [2.0; 4],
            [3.0, 2.0, 5.0, 1.0],
            BorderStyle::SOLID,
            stroke,
        ),
        bordered(
            [50.0, 4.0, 40.0, 26.0],
            [12.0; 8],
            [4.0, 1.0, 0.5, 8.0],
            [2.0; 4],
            BorderStyle::SOLID,
            stroke,
        ),
        bordered(
            [4.0, 34.0, 40.0, 26.0],
            [0.0; 8],
            [2.0; 4],
            [3.0; 4],
            BorderStyle::DASHED,
            stroke,
        ),
        bordered(
            [50.0, 34.0, 40.0, 26.0],
            [8.0; 8],
            [2.0; 4],
            [2.0, 4.0, 2.0, 4.0],
            BorderStyle::DOTTED,
            stroke,
        ),
        bordered(
            [4.0, 64.0, 40.0, 28.0],
            [6.0; 8],
            [2.0; 4],
            [4.0, 2.0, 6.0, 3.0],
            BorderStyle::SOLID,
            sides,
        ),
        bordered(
            [50.0, 64.0, 40.0, 28.0],
            wide,
            [3.0; 4],
            [3.0; 4],
            BorderStyle::DASHED,
            stroke,
        ),
    ] {
        builder.quad(quad);
    }
    compare("quads", &builder.finish(), CLOSE);
}

#[test]
fn gradients_in_every_encoding_and_shape() {
    let mut builder = FrameBuilder::new();
    white(&mut builder);
    let shapes = [
        GradientShape::Linear {
            start: [0.0, 0.0],
            end: [40.0, 10.0],
        },
        GradientShape::Radial {
            center: [20.0, 14.0],
            radius: [18.0, 10.0],
        },
        GradientShape::Conic {
            center: [20.0, 14.0],
            angle: 0.5,
        },
    ];
    let encodings = [
        StopEncoding::SRGB,
        StopEncoding::OKLAB,
        StopEncoding::LINEAR_SRGB,
    ];
    for (row, shape) in shapes.into_iter().enumerate() {
        for (column, encoding) in encodings.into_iter().enumerate() {
            let paint = builder.paint(gradient(shape, encoding, column == 2));
            let origin = [2.0 + column as f32 * 31.0, 2.0 + row as f32 * 31.0];
            builder.quad(Quad {
                paint_origin: origin,
                radii: [6.0; 8],
                ..filled([origin[0], origin[1], 29.0, 29.0], paint)
            });
        }
    }
    compare("gradients", &builder.finish(), CLOSE);
}

#[test]
fn quads_turned_scaled_and_projected() {
    let mut builder = FrameBuilder::new();
    white(&mut builder);
    let fill = builder.solid([0.0, 0.5, 0.25, 1.0]);
    let stroke = builder.solid([0.0, 0.0, 0.0, 1.0]);
    let turned = builder.space(rotation(30.0, [30.0, 30.0]));
    let scaled =
        builder.space(Matrix4::scale(2.5, 1.5, 1.0).then(&Matrix4::translation(50.0, 4.0, 0.0)));
    let mut projective = Matrix4::translation(-20.0, -20.0, 0.0)
        .then(&Matrix4::perspective(200.0))
        .then(&Matrix4::translation(70.0, 70.0, 0.0));
    projective.columns[0][3] = 0.002;
    let projected = builder.space(projective);
    for (space, bounds) in [
        (turned, [10.0, 10.0, 40.0, 30.0]),
        (scaled, [0.0, 0.0, 16.0, 16.0]),
        (projected, [0.0, 0.0, 40.0, 40.0]),
    ] {
        builder.quad(Quad {
            radii: [6.0; 8],
            border: [2.0; 4],
            stroke,
            space: SpaceRef(space),
            ..filled(bounds, fill)
        });
    }
    compare("transformed", &builder.finish(), CLOSE);
}

#[test]
fn shadows_dropped_and_inset() {
    let mut builder = FrameBuilder::new();
    white(&mut builder);
    let shadow = |bounds: [f32; 4], radii: [f32; 8], shapes: [f32; 4], blur: f32, inset: u32| {
        let grown = if inset == 0 { 3.0 * blur } else { 0.0 };
        FrameShadow {
            bounds: [
                bounds[0] - grown,
                bounds[1] - grown,
                bounds[2] + 2.0 * grown,
                bounds[3] + 2.0 * grown,
            ],
            shadow: Shadow {
                shape_bounds: [bounds[0] + 2.0, bounds[1] + 3.0, bounds[2], bounds[3]],
                radii,
                caster_bounds: bounds,
                caster_radii: radii,
                shapes,
                color: Color([0.0, 0.0, 0.0, 0.6]),
                blur,
                clip: ClipRef(FRAME_NO_CLIP),
                space: SpaceRef(0),
                inset,
            },
        }
    };
    builder.shadow(shadow([10.0, 10.0, 30.0, 24.0], [6.0; 8], [2.0; 4], 4.0, 0));
    builder.shadow(shadow(
        [56.0, 10.0, 30.0, 24.0],
        [10.0, 4.0, 10.0, 4.0, 10.0, 4.0, 10.0, 4.0],
        [4.0; 4],
        2.0,
        0,
    ));
    builder.shadow(shadow([10.0, 56.0, 30.0, 30.0], [8.0; 8], [2.0; 4], 3.0, 1));
    builder.shadow(shadow([56.0, 56.0, 30.0, 30.0], [0.0; 8], [2.0; 4], 0.0, 0));
    compare("shadows", &builder.finish(), CLOSE);
}

#[test]
fn lines_in_every_style() {
    let mut builder = FrameBuilder::new();
    white(&mut builder);
    for (index, style) in [
        LineStyle::SOLID,
        LineStyle::WAVY,
        LineStyle::DASHED,
        LineStyle::DOTTED,
        LineStyle::DOUBLE,
    ]
    .into_iter()
    .enumerate()
    {
        builder.line(Line {
            bounds: [4.3, 4.0 + index as f32 * 18.0, 85.0, 9.0],
            color: Color([0.1, 0.1, 0.6, 1.0]),
            thickness: 2.0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
            style,
            _pad: [0; 3],
        });
    }
    compare("lines", &builder.finish(), CLOSE);
}

fn group(bounds: [f32; 4], opacity: f32, filters: Vec<FrameFilter>) -> FrameGroup {
    FrameGroup {
        bounds,
        source: bounds,
        clip: FRAME_NO_CLIP,
        opacity,
        blend: BlendMode::NORMAL,
        filters: Arc::from(filters),
    }
}

#[test]
fn groups_with_opacity_and_filters() {
    let mut builder = FrameBuilder::new();
    white(&mut builder);
    let red = builder.solid([0.9, 0.1, 0.1, 1.0]);
    let blue = builder.solid([0.1, 0.2, 0.9, 1.0]);
    let invert = FrameFilter::ColorMatrix([
        -1.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 0.0,
        0.0, 1.0, 0.0,
    ]);
    let shadow = FrameFilter::DropShadow {
        offset: [3.0, 4.0],
        std_dev: 2.5,
        color: Color([0.0, 0.0, 0.3, 0.5]),
    };
    let cases = [
        (
            group([0.0, 0.0, 48.0, 48.0], 0.5, vec![]),
            [8.0, 8.0, 30.0, 30.0],
        ),
        (
            group([48.0, 0.0, 48.0, 48.0], 1.0, vec![FrameFilter::Blur(3.0)]),
            [60.0, 12.0, 24.0, 24.0],
        ),
        (
            group([0.0, 48.0, 48.0, 48.0], 1.0, vec![shadow, invert]),
            [8.0, 56.0, 28.0, 28.0],
        ),
        (
            group(
                [48.0, 48.0, 48.0, 48.0],
                0.8,
                vec![FrameFilter::Opacity(0.5), FrameFilter::Blur(1.5)],
            ),
            [56.0, 56.0, 32.0, 32.0],
        ),
    ];
    for (held, bounds) in cases {
        builder.begin(held);
        builder.quad(filled(bounds, red));
        builder.quad(filled([bounds[0] + 6.0, bounds[1] + 6.0, 12.0, 12.0], blue));
        builder.end();
    }
    compare("groups", &builder.finish(), CLOSE);
}

#[test]
fn a_backdrop_blurs_what_is_beneath_it() {
    let mut builder = FrameBuilder::new();
    white(&mut builder);
    let dark = builder.solid([0.0, 0.0, 0.0, 1.0]);
    for stripe in 0..12 {
        builder.quad(filled([stripe as f32 * 8.0, 0.0, 4.0, 96.0], dark));
    }
    let tint = builder.solid([0.2, 0.2, 0.2, 0.2]);
    let bounds = [16.0, 16.0, 64.0, 40.0];
    let filters = [FrameFilter::Blur(3.0)];
    let reach = 3.0 * 3.0 + 2.0;
    builder.backdrop(FrameBackdrop {
        backdrop: Backdrop {
            bounds,
            filters: 0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
        },
        bounds,
        source: [
            bounds[0] - reach,
            bounds[1] - reach,
            bounds[2] + 2.0 * reach,
            bounds[3] + 2.0 * reach,
        ],
        filters: Arc::from(filters.to_vec()),
    });
    builder.quad(filled(bounds, tint));
    compare("backdrop", &builder.finish(), CLOSE);
}

#[test]
fn clips_in_translated_scaled_and_turned_spaces() {
    let mut builder = FrameBuilder::new();
    white(&mut builder);
    let fill = builder.solid([0.6, 0.2, 0.0, 1.0]);
    let moved = builder.space(Matrix4::translation(4.5, 4.25, 0.0));
    let turned = builder.space(rotation(20.0, [72.0, 72.0]));
    let link = |space: u32, rect: [f32; 4], radii: [f32; 8], shapes: [f32; 4]| ClipLinkEntry {
        space,
        shape: FrameClipShape::RoundedRect {
            rect,
            radii,
            shapes,
        },
    };
    let rounded = builder.clip(ClipEntry {
        links: smallvec![
            link(moved, [0.0, 0.0, 40.0, 40.0], [12.0; 8], [2.0; 4]),
            link(0, [10.0, 10.0, 60.0, 20.0], [0.0; 8], [2.0; 4]),
        ],
    });
    let elliptic = builder.clip(ClipEntry {
        links: smallvec![link(
            0,
            [52.0, 4.0, 40.0, 40.0],
            [0.25, 3.0, 0.25, 3.0, 0.25, 3.0, 0.25, 3.0],
            [2.0; 4]
        )],
    });
    let general = builder.clip(ClipEntry {
        links: smallvec![link(turned, [52.0, 52.0, 40.0, 40.0], [10.0; 8], [4.0; 4])],
    });
    for (clip, bounds) in [
        (rounded, [0.0, 0.0, 48.0, 48.0]),
        (elliptic, [48.0, 0.0, 48.0, 48.0]),
        (general, [48.0, 48.0, 48.0, 48.0]),
    ] {
        builder.quad(Quad {
            clip: ClipRef(clip),
            ..filled(bounds, fill)
        });
    }
    compare("clips", &builder.finish(), CLOSE);
}
