//! Two- and four-component vectors as arrays.

/// Returns the length of `v`.
pub(crate) fn length(v: [f32; 2]) -> f32 {
    (v[0] * v[0] + v[1] * v[1]).sqrt()
}

/// Returns the dot product of `a` and `b`.
pub(crate) fn dot(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

/// Returns `a` scaled by `s`.
pub(crate) fn scale4(a: [f32; 4], s: f32) -> [f32; 4] {
    [a[0] * s, a[1] * s, a[2] * s, a[3] * s]
}

/// Returns `a + (b - a) × t` per component.
pub(crate) fn mix4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

/// Returns `above` over `below`, both premultiplied.
pub(crate) fn over(below: [f32; 4], above: [f32; 4]) -> [f32; 4] {
    let keep = 1.0 - above[3];
    [
        above[0] + below[0] * keep,
        above[1] + below[1] * keep,
        above[2] + below[2] * keep,
        above[3] + below[3] * keep,
    ]
}

/// Returns the straight colour of a premultiplied colour, or its components as they are at zero
/// alpha.
pub(crate) fn straight_rgb(color: [f32; 4]) -> [f32; 3] {
    if color[3] <= 0.0 {
        return [color[0], color[1], color[2]];
    }
    [
        color[0] / color[3],
        color[1] / color[3],
        color[2] / color[3],
    ]
}
