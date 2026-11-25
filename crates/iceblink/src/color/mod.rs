//! Colour spaces, interpolation between them, and the conversion to renderer input.
//!
//! A [`Color`] is three channels and an alpha in one of fourteen [colour spaces](ColorSpace), with
//! the names and conversions of CSS Color 4. [`Color::to_premultiplied_srgb`] converts any colour
//! into the premultiplied, gamma-encoded sRGB a renderer draws with, which is the
//! [`Color`](crate::Color) of items and paints.
//!
//! | Module | Contents |
//! |---|---|
//! | [`space`] | [`ColorSpace`], the fourteen spaces, and what their channels mean |
//! | [`value`] | [`Color`], conversion between spaces, and the two premultiplications |
//! | [`mod@interpolate`] | [`interpolate()`], [`Interpolation`] and [`HueInterpolation`] |
//! | [`mod@mix`] | [`color_mix`] and [`color_mix_evenly`] |
//! | [`gradient`] | [`GradientStop`] and [`densify()`] |
//! | [`mod@gamma`] | [`gamma_correction_ratios`] for text rendering |
//!
//! ```
//! use iceblink::color::{Color, ColorSpace, Interpolation, interpolate};
//!
//! let blue = Color::srgb(0.0, 0.0, 1.0, 1.0);
//! let yellow = Color::srgb(1.0, 1.0, 0.0, 1.0);
//! let middle = interpolate(blue, yellow, 0.5, Interpolation::new(ColorSpace::Oklab));
//!
//! let [red, green, blue, alpha] = middle.to_premultiplied_srgb();
//! assert!(alpha == 1.0 && red > 0.0 && green > 0.0 && blue > 0.0);
//! ```

mod convert;
pub mod gamma;
pub mod gradient;
pub mod interpolate;
pub mod mix;
pub mod space;
pub mod value;

pub use crate::color::gamma::gamma_correction_ratios;
pub use crate::color::gradient::{
    DEFAULT_TOLERANCE, GradientStop, densify, densify_with_tolerance,
};
pub use crate::color::interpolate::{HueInterpolation, Interpolation, interpolate};
pub use crate::color::mix::{color_mix, color_mix_evenly};
pub use crate::color::space::{ColorSpace, WhitePoint};
pub use crate::color::value::{Color, PremultipliedLinear};
