//! Stand-ins for the parts of a compositor stack a test does not exercise: a glyph rasteriser, a
//! font registry and a renderer.

mod fonts;
mod raster;
mod renderer;

pub use self::fonts::{FakeFontRegistry, fake_font};
pub use self::raster::FakeGlyphRaster;
pub use self::renderer::{Draw, NullRenderer};
