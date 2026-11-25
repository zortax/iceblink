//! Golden text files.

use std::fs;
use std::path::Path;

use crate::golden::diff::first_difference;
use crate::golden::settle::{Outcome, settle};

/// Compares `actual` with the text in the file at `path`.
pub fn compare_text(path: &Path, actual: &str) -> Outcome {
    let Ok(expected) = fs::read_to_string(path) else {
        return Outcome::Missing;
    };
    match first_difference(&expected, actual) {
        None => Outcome::Matched,
        Some(report) => Outcome::Differed(report),
    }
}

/// Asserts that `actual` is the text of the golden at `path`.
///
/// With `ICEBLINK_BLESS` set, a missing or differing golden is written and the test still fails.
///
/// # Panics
///
/// Panics when the golden is missing or differs.
#[track_caller]
pub fn assert_text(path: &Path, actual: &str) {
    settle(path, compare_text(path, actual), |path| {
        fs::write(path, actual)
    });
}
