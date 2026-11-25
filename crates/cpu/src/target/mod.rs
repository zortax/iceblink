//! The images the renderer draws into: pixmaps of eight-bit texels, canvases that hold eight-bit
//! or float texels, and the views drawing and sampling go through.

mod canvas;
mod pixmap;
mod surface;
mod texture;

pub use self::pixmap::Pixmap;

pub(crate) use self::canvas::{Canvas, Precision};
pub(crate) use self::surface::{Surface, TexelsMut};
pub(crate) use self::texture::Texture;

/// Returns `value` as an eight-bit channel, rounded to the nearest step.
pub(crate) fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Returns an eight-bit channel as a value from zero to one.
pub(crate) fn from_byte(value: u8) -> f32 {
    f32::from(value) / 255.0
}
