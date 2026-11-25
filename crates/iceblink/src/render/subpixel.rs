//! The order of a display's colour stripes.

/// The order of a display's colour stripes, from left to right.
///
/// Subpixel sprites hold one coverage per stripe, and the order decides which channel each
/// coverage is drawn into.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SubpixelOrder {
    /// Red, green, blue.
    #[default]
    RedToBlue,
    /// Blue, green, red.
    BlueToRed,
}
