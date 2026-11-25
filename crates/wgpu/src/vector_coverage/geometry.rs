//! Outlines flattened into the line segments a sample is tested against.

use kurbo::{Affine, BezPath, PathEl, Point, Shape as _, Stroke, StrokeOpts};

/// How far a flattened polyline may stray from its curve, in device pixels.
// Under half the quarter pixel between the rows and columns of samples.
pub(crate) const TOLERANCE: f64 = 0.1;

/// One line segment of an outline, as `[x0, y0, x1, y1]` in device pixels.
pub(crate) type Segment = [f32; 4];

/// Returns the tolerance in path units that keeps a path drawn under `transform` within
/// [`TOLERANCE`] device pixels of its curves.
///
/// A transform that scales by less than a thousandth counts as scaling by a thousandth.
pub(crate) fn tolerance(transform: Affine) -> f64 {
    let [a, b, c, d, _, _] = transform.as_coeffs();
    TOLERANCE / a.hypot(b).max(c.hypot(d)).max(1.0e-3)
}

/// Appends the outline of `path`, mapped by `transform`, to `into` as closed polylines.
///
/// `tolerance` is in the units of `path`. Every subpath is closed, and horizontal segments are
/// left out.
// A winding test is only defined on closed outlines, and a horizontal segment crosses no row.
pub(crate) fn flatten(path: &BezPath, transform: Affine, tolerance: f64, into: &mut Vec<Segment>) {
    let mut start: Option<Point> = None;
    let mut cursor = Point::ZERO;
    kurbo::flatten(path.iter(), tolerance, |element| match element {
        PathEl::MoveTo(point) => {
            close(start, cursor, transform, into);
            start = Some(point);
            cursor = point;
        }
        PathEl::LineTo(point) => {
            push(cursor, point, transform, into);
            cursor = point;
        }
        PathEl::ClosePath => {
            if let Some(first) = start {
                push(cursor, first, transform, into);
                cursor = first;
            }
            start = None;
        }
        // Flattening yields only the three elements above.
        PathEl::QuadTo(..) | PathEl::CurveTo(..) => {}
    });
    close(start, cursor, transform, into);
}

/// Appends the outline of `path` stroked with `style`, mapped by `transform`, to `into`.
///
/// The stroke is expanded with its width, caps, joins, miter limit and dashes in the units of
/// `path`, and then mapped, so a transform that stretches one axis stretches the pen with it.
pub(crate) fn flatten_stroke(
    path: &BezPath,
    style: &Stroke,
    transform: Affine,
    tolerance: f64,
    into: &mut Vec<Segment>,
) {
    let widened = Stroke {
        width: style.width.max(0.0),
        ..style.clone()
    };
    let stroked = kurbo::stroke(
        path.path_elements(tolerance),
        &widened,
        &StrokeOpts::default(),
        tolerance,
    );
    flatten(&stroked, transform, tolerance, into);
}

/// Closes an open subpath back to where it began.
fn close(start: Option<Point>, cursor: Point, transform: Affine, into: &mut Vec<Segment>) {
    if let Some(first) = start
        && first != cursor
    {
        push(cursor, first, transform, into);
    }
}

/// Appends one segment, unless it is horizontal.
fn push(from: Point, to: Point, transform: Affine, into: &mut Vec<Segment>) {
    let from = transform * from;
    let to = transform * to;
    if from.y == to.y {
        return;
    }
    into.push([from.x as f32, from.y as f32, to.x as f32, to.y as f32]);
}

#[cfg(test)]
mod tests {
    use kurbo::{Affine, BezPath, Circle, Shape as _, Stroke};

    use super::{TOLERANCE, flatten, flatten_stroke, tolerance};

    fn line(length: f64) -> BezPath {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((0.0, length));
        path
    }

    #[test]
    fn an_unclosed_subpath_is_closed() {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((10.0, 0.0));
        path.line_to((10.0, 10.0));
        let mut segments = Vec::new();

        flatten(&path, Affine::IDENTITY, TOLERANCE, &mut segments);

        // The right edge and the closing diagonal, without the horizontal edge.
        assert_eq!(segments.len(), 2);
        assert!(segments.iter().any(|s| [s[2], s[3]] == [0.0, 0.0]));
    }

    #[test]
    fn a_stroke_becomes_its_outline() {
        let mut outline = Vec::new();

        flatten_stroke(
            &line(20.0),
            &Stroke::new(4.0),
            Affine::IDENTITY,
            TOLERANCE,
            &mut outline,
        );

        let widest = outline
            .iter()
            .flat_map(|s| [s[0], s[2]])
            .fold(f32::MIN, f32::max);
        assert!((widest - 2.0).abs() < 0.01, "{widest}");
    }

    #[test]
    fn a_dashed_stroke_has_more_edges_than_the_solid_one() {
        let mut solid = Vec::new();
        let mut dashed = Vec::new();
        let dashes = Stroke::new(4.0).with_dashes(0.0, [4.0, 4.0]);

        flatten_stroke(
            &line(40.0),
            &Stroke::new(4.0),
            Affine::IDENTITY,
            TOLERANCE,
            &mut solid,
        );
        flatten_stroke(
            &line(40.0),
            &dashes,
            Affine::IDENTITY,
            TOLERANCE,
            &mut dashed,
        );

        assert!(dashed.len() > solid.len());
    }

    #[test]
    fn a_small_path_drawn_large_stays_within_the_tolerance() {
        let circle = Circle::new((0.0, 0.0), 1.0).into_path(1.0e-6);
        let scale = Affine::scale(64.0);
        let mut segments = Vec::new();

        flatten(&circle, scale, tolerance(scale), &mut segments);

        let worst = segments
            .iter()
            .map(|s| {
                let x = f64::from(s[0] + s[2]) / 2.0;
                let y = f64::from(s[1] + s[3]) / 2.0;
                (64.0 - x.hypot(y)).abs()
            })
            .fold(0.0_f64, f64::max);
        assert!(
            worst <= TOLERANCE,
            "a chord strays {worst} px from the curve"
        );
    }
}
