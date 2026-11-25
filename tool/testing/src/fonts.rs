//! The font files the tests read: Noto Sans, a colour face and a variable face.

use iceblink::FontData;

/// A Latin text face with outlines.
pub const LATIN: &str = "NotoSans-Regular.ttf";

/// A face with colour layers.
pub const COLOR: &str = "NotoZnamennyMusicalNotation-Regular.ttf";

/// A variable face with a weight axis, subset to a few letters.
pub const VARIABLE: &str = "RubikSubset-Variable.ttf";

/// Reads the test font `file`, one of [`LATIN`], [`COLOR`] and [`VARIABLE`].
///
/// # Panics
///
/// Panics when the file cannot be read.
pub fn read_font(file: &str) -> FontData {
    let path = format!("{}/fonts/{file}", env!("CARGO_MANIFEST_DIR"));
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("reading {path}: {error}"));
    FontData::new(bytes, 0)
}
