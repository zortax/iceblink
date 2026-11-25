//! Golden images, compared within a declared tolerance.

use std::path::Path;

use crate::golden::RgbaImage;
use crate::golden::png::{read_png, write_png};
use crate::golden::settle::{Outcome, settle};

/// How far a rendering may differ from its golden image.
///
/// A pixel differs when any channel differs by more than `max_channel`. The rendering matches
/// when at most `max_pixels` pixels differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tolerance {
    /// The largest channel difference a pixel may have and still count as equal.
    pub max_channel: u8,
    /// How many pixels may differ.
    pub max_pixels: u64,
}

impl Tolerance {
    /// No difference at all.
    pub const EXACT: Self = Self {
        max_channel: 0,
        max_pixels: 0,
    };
}

/// How two images of one size differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Difference {
    /// The largest channel difference of any pixel.
    pub max_channel: u8,
    /// How many pixels differ by more than the tolerance's channel difference.
    pub pixels: u64,
    /// The first such pixel, row by row.
    pub first: Option<(u32, u32)>,
}

/// Measures how `actual` differs from `expected` under `tolerance`, or returns `None` when the
/// sizes differ.
pub fn difference(
    expected: &RgbaImage,
    actual: &RgbaImage,
    tolerance: Tolerance,
) -> Option<Difference> {
    if (expected.width(), expected.height()) != (actual.width(), actual.height()) {
        return None;
    }

    let mut found = Difference {
        max_channel: 0,
        pixels: 0,
        first: None,
    };
    let pairs = expected
        .pixels()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(actual.pixels().as_chunks::<4>().0);
    for (index, (left, right)) in pairs.enumerate() {
        let worst = left
            .iter()
            .zip(right)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        found.max_channel = found.max_channel.max(worst);
        if worst > tolerance.max_channel {
            found.pixels += 1;
            if found.first.is_none() {
                let index = index as u32;
                found.first = Some((index % actual.width(), index / actual.width()));
            }
        }
    }
    Some(found)
}

/// Compares `actual` with the golden PNG at `path` under `tolerance`.
pub fn compare_png(path: &Path, actual: &RgbaImage, tolerance: Tolerance) -> Outcome {
    let Ok(expected) = read_png(path) else {
        return Outcome::Missing;
    };
    let Some(found) = difference(&expected, actual, tolerance) else {
        return Outcome::Differed(format!(
            "the golden is {}x{} and the rendering {}x{}",
            expected.width(),
            expected.height(),
            actual.width(),
            actual.height()
        ));
    };
    if found.pixels <= tolerance.max_pixels {
        return Outcome::Matched;
    }

    let (x, y) = found.first.unwrap_or_default();
    Outcome::Differed(format!(
        "{} pixels differ, {} allowed; the largest channel difference is {}, {} allowed; the \
         first is at ({x}, {y}): golden {:?}, rendering {:?}",
        found.pixels,
        tolerance.max_pixels,
        found.max_channel,
        tolerance.max_channel,
        expected.pixel(x, y),
        actual.pixel(x, y)
    ))
}

/// Asserts that `actual` matches the golden PNG at `path` under `tolerance`.
///
/// With `ICEBLINK_BLESS` set, a missing or differing golden is written and the test still fails.
///
/// # Panics
///
/// Panics when the golden is missing or differs beyond the tolerance.
#[track_caller]
pub fn assert_png(path: &Path, actual: &RgbaImage, tolerance: Tolerance) {
    settle(path, compare_png(path, actual, tolerance), |path| {
        write_png(path, actual)
    });
}
