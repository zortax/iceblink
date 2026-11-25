//! Golden files: text and PNG images, compared with a rendering and rewritten on request.
//!
//! A comparison fails when the golden is missing or differs. With `ICEBLINK_BLESS=1`, it
//! writes the rendering as the new golden and still fails, so no golden passes in the run that
//! writes it.

mod bless;
mod compare;
mod diff;
mod image;
mod png;
mod settle;
mod text;

pub use self::bless::{BLESS, is_blessing};
pub use self::compare::{Difference, Tolerance, assert_png, compare_png, difference};
pub use self::diff::first_difference;
pub use self::image::RgbaImage;
pub use self::png::{read_png, write_png};
pub use self::settle::Outcome;
pub use self::text::{assert_text, compare_text};
