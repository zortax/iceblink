//! The fonts a client and a compositor share.

use crate::{FontData, FontId};

/// The fonts a client's layout and a compositor's glyph cache share, so a font has one `FontId`
/// in both.
///
/// Fonts are never removed.
pub trait FontRegistry: Send + Sync {
    /// Registers a font and returns its id.
    ///
    /// Registering equal bytes and index again returns the same id.
    fn register(&self, data: FontData) -> FontId;

    /// Returns the font of `id`, or `None` when no font has that id.
    fn get(&self, id: FontId) -> Option<FontData>;

    /// Returns a number that increases whenever a font is registered.
    fn generation(&self) -> u64;
}
