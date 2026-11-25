//! Checks that the numbers and colours commands carry are well formed.

mod geometry;
mod nodes;
mod paint;

pub(crate) use self::geometry::{path, stroke};
pub(crate) use self::nodes::{clip_shape, effect, filters, matrix, params, scroll, transform};
pub(crate) use self::paint::{color, paint};

/// Reports whether every value is finite.
pub(crate) fn all<'a>(values: impl IntoIterator<Item = &'a f32>) -> bool {
    values.into_iter().all(|value| value.is_finite())
}
