//! The test fonts and keys over them, and the typesetter of the layout tests.

// Every test binary compiles this module and uses a subset of it.
#![allow(dead_code)]

#[cfg(feature = "typeset")]
pub(crate) mod typeset;

use iceblink::{FontData, FontId, GlyphBitmap, GlyphKey, OutlineKey, RasterStyle};
use skrifa::MetadataProvider;

/// A Latin text face with outlines.
pub(crate) const LATIN: &str = iceblink_testing::fonts::LATIN;

/// A face with colour layers.
pub(crate) const COLOR: &str = iceblink_testing::fonts::COLOR;

/// A variable face with a weight axis, subset to a few letters.
pub(crate) const VARIABLE: &str = iceblink_testing::fonts::VARIABLE;

/// Reads one of the test fonts.
pub(crate) fn font(file: &str) -> FontData {
    iceblink_testing::fonts::read_font(file)
}

/// Returns the glyph `character` maps to in `font`.
pub(crate) fn glyph(font: &FontData, character: char) -> u32 {
    let face = skrifa::FontRef::new(font.bytes()).expect("a test font parses");
    face.charmap()
        .map(character)
        .unwrap_or_else(|| panic!("the font maps {character:?}"))
        .to_u32()
}

/// Returns the key of `glyph` at `size` device pixels, unhinted, at phase zero, in `style`.
pub(crate) fn key(glyph: u32, size: f32, style: RasterStyle) -> GlyphKey {
    GlyphKey {
        font: FontId(0),
        glyph,
        size_bits: size.to_bits(),
        instance: 0,
        phase: 0,
        style,
        hint: false,
        palette: 0,
        embolden_bits: 0.0f32.to_bits(),
        skew_bits: 0.0f32.to_bits(),
    }
}

/// Returns the outline key of `glyph` at `size` device pixels.
pub(crate) fn outline_key(glyph: u32, size: f32) -> OutlineKey {
    OutlineKey {
        font: FontId(0),
        glyph,
        size_bits: size.to_bits(),
        instance: 0,
        skew_bits: 0.0f32.to_bits(),
    }
}

/// Returns the summed coverage of a mono bitmap.
pub(crate) fn ink(bitmap: &GlyphBitmap) -> u64 {
    bitmap.bytes.iter().map(|byte| u64::from(*byte)).sum()
}
