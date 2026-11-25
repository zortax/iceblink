//! The content a window shows.

use std::sync::Arc;

use iceblink::Transaction;
use iceblink_text::typeset::FontRegistry;

use crate::scene::Scene;

/// Content that records itself into a scene and changes over time or on input.
///
/// Sizes and positions are in logical pixels.
pub trait Demo: Sized {
    /// The window title.
    const TITLE: &'static str;

    /// Returns the font registry the compositor shares with the demo's layout, if the demo lays
    /// out text in fonts of its own.
    fn fonts() -> Option<Arc<FontRegistry>> {
        None
    }

    /// Records the content for an output of `size`.
    fn new(scene: &mut Scene, tx: &mut Transaction, size: [f32; 2]) -> Self;

    /// Lays the content out again for an output of `size`.
    fn resize(&mut self, scene: &Scene, tx: &mut Transaction, size: [f32; 2]);

    /// Writes the content as it is `seconds` after the window opened, and reports whether it
    /// changes again later.
    fn advance(&mut self, _tx: &mut Transaction, _seconds: f32) -> bool {
        false
    }

    /// Scrolls the content by `delta`, where a positive `y` reveals content further down.
    fn wheel(&mut self, _tx: &mut Transaction, _delta: [f32; 2]) {}
}
