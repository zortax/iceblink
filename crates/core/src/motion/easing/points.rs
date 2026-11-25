//! The CSS `linear()` easing.

/// Returns the piecewise-linear easing through `points`, each `[input, output]` with
/// non-decreasing inputs, at progress `x`.
///
/// The easing holds the first output before the first input and the last output after the last.
/// With no points it is the identity.
pub fn linear_points(points: &[[f32; 2]], x: f32) -> f32 {
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return x;
    };
    if x < first[0] {
        return first[1];
    }
    if x >= last[0] {
        return last[1];
    }

    // The last point at or before `x`, which is followed by one past it.
    let at = points.partition_point(|point| point[0] <= x) - 1;
    let [from, to] = [points[at], points[at + 1]];
    let share = (x - from[0]) / (to[0] - from[0]);
    from[1] + (to[1] - from[1]) * share
}
