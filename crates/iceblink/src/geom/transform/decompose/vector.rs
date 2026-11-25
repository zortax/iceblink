//! Arithmetic on three-component vectors.

/// Returns the Euclidean length of a vector.
pub(super) fn length(vector: [f32; 3]) -> f32 {
    dot(vector, vector).sqrt()
}

/// Returns the dot product of two vectors.
pub(super) fn dot(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

/// Returns the cross product of two vectors.
pub(super) fn cross(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

/// Returns `vector + factor * other`.
pub(super) fn combine(vector: [f32; 3], other: [f32; 3], factor: f32) -> [f32; 3] {
    [
        vector[0] + other[0] * factor,
        vector[1] + other[1] * factor,
        vector[2] + other[2] * factor,
    ]
}

/// Divides the vector by `length`, or returns `None` if `length` is zero or not finite.
pub(super) fn unit_vector(vector: [f32; 3], length: f32) -> Option<[f32; 3]> {
    if length == 0.0 || !length.is_finite() {
        return None;
    }
    let inverse = length.recip();
    Some([
        vector[0] * inverse,
        vector[1] * inverse,
        vector[2] * inverse,
    ])
}
