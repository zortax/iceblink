//! The region a filtered composite reads.

use crate::node::Filter;

/// Returns every device pixel a composite of `bounds` under `filters` reads.
///
/// `bounds` and the result are device rectangles as `[x, y, width, height]`. Each side of `bounds`
/// is outset by the sum of the filters' supports on that side. With `r(σ) = 3 max(σ, 0) + 2`, the
/// supports are:
///
/// - `Blur(σ)` with `σ > 0` reads `r(σ)` on every side.
/// - `DropShadow` with offset `(dx, dy)` and deviation `σ` reads `max(r(σ) - dx, 0)` on the left,
///   `max(r(σ) - dy, 0)` on the top, `max(r(σ) + dx, 0)` on the right and `max(r(σ) + dy, 0)` on
///   the bottom.
/// - `Shader` reads `max(reach, 0)` on every side.
/// - Every other filter reads nothing outside `bounds`.
///
/// When every sum is zero, the result is `bounds`.
pub fn read_extent(bounds: [f32; 4], filters: &[Filter]) -> [f32; 4] {
    let [mut left, mut top, mut right, mut bottom] = [0.0f32; 4];
    for filter in filters {
        let [l, t, r, b] = filter.support();
        left += l;
        top += t;
        right += r;
        bottom += b;
    }

    if left == 0.0 && top == 0.0 && right == 0.0 && bottom == 0.0 {
        return bounds;
    }

    let [x, y, width, height] = bounds;
    [
        x - left,
        y - top,
        width + left + right,
        height + top + bottom,
    ]
}
