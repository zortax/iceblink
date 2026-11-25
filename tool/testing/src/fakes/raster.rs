//! A glyph rasteriser that needs no font files.

use std::sync::atomic::{AtomicU64, Ordering};

use iceblink::GlyphRaster;
use iceblink::kurbo::{Affine, BezPath, Rect, Shape};
use iceblink::{FontData, GlyphBitmap, GlyphKey, GlyphPlacement, OutlineKey, PHASES, RasterStyle};

/// A glyph rasteriser over no font at all, which draws every glyph as a filled box.
///
/// The box sits on the baseline from the origin, `round(size / 2)` pixels wide and
/// `round(size × 3 / 4)` high, and is the same in every font. The glyph id decides the rest:
///
/// - Glyph [`BLANK`](Self::BLANK) is blank, with a zero-sized bitmap and an empty outline.
/// - Glyphs from [`MISSING`](Self::MISSING) up are not in the font.
/// - Glyphs from [`COLOR`](Self::COLOR) up to `MISSING` are colour glyphs.
/// - Every other glyph is a plain box.
///
/// A phase above zero moves the box right by `phase / 4` pixels, which adds a column of partial
/// coverage on each side. Subpixel bitmaps repeat the coverage in all three channels. Colour
/// bitmaps are red, with the key's palette as green, and the coverage as alpha. Emboldening and
/// slant leave bitmaps unchanged, and slant shears the outline.
///
/// A mono bitmap without slant covers exactly the box `PenPosition::bitmap_box` gives for the
/// bounds of the outline, so a client computes a fake glyph's ink from [`box_size`](Self::box_size)
/// at the device size. Every other bitmap lies inside that box.
#[derive(Debug, Default)]
pub struct FakeGlyphRaster {
    rasterised: AtomicU64,
}

impl FakeGlyphRaster {
    /// The blank glyph, a space.
    pub const BLANK: u32 = 32;

    /// The first colour glyph.
    pub const COLOR: u32 = 0x1_F000;

    /// The first glyph the font lacks.
    pub const MISSING: u32 = 0x11_0000;

    /// Returns a rasteriser that has rasterised nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns how many bitmaps have been rasterised, blank and missing glyphs included.
    pub fn rasterised(&self) -> u64 {
        self.rasterised.load(Ordering::Relaxed)
    }

    /// Returns the width and height of the box at the device size `size`.
    pub fn box_size(size: f32) -> (u32, u32) {
        let width = (size / 2.0).round().max(0.0) as u32;
        let height = (size * 0.75).round().max(0.0) as u32;
        (width, height)
    }
}

impl GlyphRaster for FakeGlyphRaster {
    fn rasterize(
        &self,
        _font: &FontData,
        key: &GlyphKey,
        _coords: &[i16],
        out: &mut GlyphBitmap,
    ) -> Option<GlyphPlacement> {
        self.rasterised.fetch_add(1, Ordering::Relaxed);
        out.bytes.clear();
        if key.glyph >= Self::MISSING {
            out.width = 0;
            out.height = 0;
            return None;
        }

        let (width, height) = if key.glyph == Self::BLANK {
            (0, 0)
        } else {
            Self::box_size(key.size())
        };
        let columns = coverage_columns(width, key.phase);
        out.width = if height == 0 { 0 } else { columns.len() as u32 };
        out.height = if out.width == 0 { 0 } else { height };
        for _ in 0..out.height {
            for coverage in &columns {
                push_pixel(out, key, *coverage);
            }
        }

        Some(GlyphPlacement {
            left: 0,
            top: out.height as i32,
        })
    }

    fn is_color(&self, _font: &FontData, glyph: u32) -> bool {
        (Self::COLOR..Self::MISSING).contains(&glyph)
    }

    fn outline(
        &self,
        _font: &FontData,
        key: &OutlineKey,
        _coords: &[i16],
        out: &mut BezPath,
    ) -> bool {
        out.truncate(0);
        if key.glyph >= Self::MISSING {
            return false;
        }
        if key.glyph == Self::BLANK {
            return true;
        }

        let (width, height) = Self::box_size(key.size());
        let mut path = Rect::new(0.0, -f64::from(height), f64::from(width), 0.0).to_path(0.1);
        let lean = f64::from(key.skew().to_radians().tan());
        if lean != 0.0 {
            // Points move right in proportion to their height above the baseline.
            path.apply_affine(Affine::new([1.0, 0.0, -lean, 1.0, 0.0, 0.0]));
        }
        out.extend(path);
        true
    }
}

/// Returns the coverage of each column of a box `width` columns wide, moved right by `phase`
/// quarters.
fn coverage_columns(width: u32, phase: u8) -> Vec<u8> {
    let phase = u32::from(phase % PHASES);

    if width == 0 {
        return Vec::new();
    }
    if phase == 0 {
        return vec![u8::MAX; width as usize];
    }

    let quarters = u32::from(PHASES);
    let mut columns = vec![u8::MAX; width as usize + 1];
    columns[0] = (255 * (quarters - phase) / quarters) as u8;
    columns[width as usize] = (255 * phase / quarters) as u8;
    columns
}

fn push_pixel(out: &mut GlyphBitmap, key: &GlyphKey, coverage: u8) {
    match key.style {
        RasterStyle::Mono => out.bytes.push(coverage),
        RasterStyle::Subpixel => out.bytes.extend_from_slice(&[coverage; 3]),
        RasterStyle::Color => {
            out.bytes
                .extend_from_slice(&[u8::MAX, key.palette as u8, 0, coverage]);
        }
    }
}
