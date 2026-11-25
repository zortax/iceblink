//! Glyph rasterisation over swash and skrifa, and text layout over Parley.
//!
//! [`SwashRaster`] implements [`iceblink::GlyphRaster`]: hinted and unhinted bitmaps at the four
//! subpixel phases, subpixel coverage, colour glyphs from colour layers and colour strikes, and
//! glyph outlines for the path route.
//!
//! The `typeset` feature adds the `typeset` module, which lays text out over Parley and records it
//! as compositor items.

#[cfg(feature = "typeset")]
pub mod typeset;

mod color;
mod outline;
mod raster;

pub use crate::raster::SwashRaster;
