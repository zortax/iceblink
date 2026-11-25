//! Frame paths: fills by both rules, path clips, and strokes, one of them in a turned space.

use std::sync::Arc;

use iceblink::geom::{Point, Rect, Size};
use iceblink::kurbo::{self, Affine, BezPath, Cap, Join, Shape, Stroke};
use iceblink::{FRAME_NO_CLIP, FillRule, FramePath, PathClip, PathFill, PathStroke};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, filled, opaque};
use crate::conformance::draw::canvas::{Canvas, SIZE, golden, skip};
use crate::conformance::draw::cases::cell;
use crate::golden::Tolerance;

/// Rasterised edges: a pixel may differ by a few steps where its coverage is partial.
// Each rasteriser approximates the covered area.
const EDGES: Tolerance = Tolerance {
    max_channel: 6,
    max_pixels: 96,
};

/// Stroke edges: an edge pixel's coverage may differ by up to 0.2.
// A rasteriser may place a stroke's outline up to a fifth of a pixel from where another one
// places it.
const STROKE_EDGES: Tolerance = Tolerance {
    max_channel: 51,
    max_pixels: 0,
};

/// Stroke edge pixels: no more pixels than lie on the strokes' edges differ by more than 2 steps.
// vello_cpu against the golden: 82 pixels.
const STROKE_EDGE_PIXELS: Tolerance = Tolerance {
    max_channel: 2,
    max_pixels: 128,
};

/// Returns a five-pointed star in a cell, which crosses itself.
fn star([x, y, width, height]: [f32; 4]) -> BezPath {
    let centre = (f64::from(x + width / 2.0), f64::from(y + height / 2.0));
    let radius = f64::from(width.min(height)) / 2.0 - 2.0;
    let mut path = BezPath::new();
    for step in 0..5 {
        let angle = std::f64::consts::PI * (-0.5 + 0.8 * f64::from(step));
        let point = (
            centre.0 + radius * angle.cos(),
            centre.1 + radius * angle.sin(),
        );
        if step == 0 {
            path.move_to(point);
        } else {
            path.line_to(point);
        }
    }
    path.close_path();
    path
}

fn device(cell: [f32; 4]) -> iceblink::DeviceRect {
    let [x, y, width, height] = cell;
    Rect::new(
        Point::new(x.floor() as i32, y.floor() as i32),
        Size::new(width.ceil() as i32 + 1, height.ceil() as i32 + 1),
    )
}

fn path(path: BezPath, ink: [f32; 4]) -> FramePath {
    FramePath {
        path: Arc::new(path),
        transform: Affine::IDENTITY,
        brush: Affine::IDENTITY,
        fill: None,
        stroke: None,
        clips: Arc::from(Vec::new()),
        ink: device(ink),
    }
}

/// Opens a canvas on a target from `make` that rasterises paths, or reports the case skipped.
fn open(make: MakeTarget<'_>, case: &str) -> Option<Canvas> {
    let mut canvas = Canvas::open(make)?;
    let status = canvas.vector_status();
    if !status.initialized && status.name.is_empty() {
        skip(case, "the renderer has no path rasteriser");
        return None;
    }
    Some(canvas)
}

/// A star filled by the non-zero and the even-odd rule, and a fill cut by a path clip.
pub fn paths(make: MakeTarget<'_>) {
    let Some(mut canvas) = open(make, "paths") else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], white));
    let green = builder.solid(opaque(20, 140, 60));

    let mut non_zero = path(star(cell(0)), cell(0));
    non_zero.fill = Some(PathFill {
        paint: green,
        rule: FillRule::NON_ZERO,
    });
    builder.path(non_zero, FRAME_NO_CLIP);

    let mut even_odd = path(star(cell(1)), cell(1));
    even_odd.fill = Some(PathFill {
        paint: green,
        rule: FillRule::EVEN_ODD,
    });
    builder.path(even_odd, FRAME_NO_CLIP);

    let [x, y, width, height] = cell(3);
    let disc = kurbo::Circle::new(
        (f64::from(x + width / 2.0), f64::from(y + height / 2.0)),
        f64::from(width / 2.0),
    );
    let mut clipped = path(
        kurbo::Rect::new(
            f64::from(x),
            f64::from(y),
            f64::from(x + width),
            f64::from(y + height),
        )
        .to_path(0.1),
        cell(3),
    );
    clipped.fill = Some(PathFill {
        paint: green,
        rule: FillRule::NON_ZERO,
    });
    clipped.clips = Arc::from(vec![PathClip {
        path: Arc::new(disc.to_path(0.1)),
        rule: FillRule::NON_ZERO,
    }]);
    builder.path(clipped, FRAME_NO_CLIP);

    golden("paths", &canvas.draw(builder.finish()), EDGES);
}

/// A stroked curve with round caps and joins, and a stroked rectangle in a turned space.
pub fn path_strokes(make: MakeTarget<'_>) {
    let Some(mut canvas) = open(make, "path_strokes") else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], white));
    let purple = builder.solid(opaque(120, 30, 160));

    let [x, y, width, height] = cell(2);
    let mut curve = BezPath::new();
    curve.move_to((f64::from(x) + 4.0, f64::from(y + height) - 6.0));
    curve.curve_to(
        (f64::from(x) + 10.0, f64::from(y) - 4.0),
        (f64::from(x + width) - 10.0, f64::from(y + height) + 4.0),
        (f64::from(x + width) - 4.0, f64::from(y) + 6.0),
    );
    let mut stroked = path(curve, cell(2));
    stroked.stroke = Some(PathStroke {
        paint: purple,
        style: Stroke::new(3.0)
            .with_caps(Cap::Round)
            .with_join(Join::Round),
    });
    builder.path(stroked, FRAME_NO_CLIP);

    let [x, y, width, height] = cell(4);
    let mut turned = path(
        kurbo::Rect::new(-10.0, -10.0, 10.0, 10.0).to_path(0.1),
        cell(4),
    );
    turned.transform = Affine::translate((f64::from(x + width / 2.0), f64::from(y + height / 2.0)))
        * Affine::rotate(0.5);
    turned.stroke = Some(PathStroke {
        paint: purple,
        style: Stroke::new(2.0),
    });
    builder.path(turned, FRAME_NO_CLIP);

    let image = canvas.draw(builder.finish());
    golden("path_strokes", &image, STROKE_EDGES);
    golden("path_strokes", &image, STROKE_EDGE_PIXELS);
}
