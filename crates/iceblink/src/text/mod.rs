//! Glyph rasterisation: the keys glyphs are rasterised and cached by, the bitmaps a rasteriser
//! writes, and the split of a glyph's position into a pixel and a phase.

mod bitmap;
mod key;
mod pen;

pub use self::bitmap::{GlyphBitmap, GlyphPlacement};
pub use self::key::{GlyphKey, OutlineKey, RasterStyle};
pub use self::pen::{PHASES, PenPosition};
