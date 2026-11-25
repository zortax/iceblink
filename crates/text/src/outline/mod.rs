//! Glyph outlines as curves.
//!
//! Outlines are unhinted and unshifted, and a slant leans each letter about the baseline.

mod pen;

use std::sync::Mutex;

use iceblink::kurbo::BezPath;
use iceblink::{FontData, OutlineKey};
use rustc_hash::FxHashMap;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::DrawSettings;
use skrifa::raw::TableProvider;
use skrifa::raw::types::F2Dot14;
use skrifa::{FontRef, GlyphId, MetadataProvider};

use crate::outline::pen::Pen;

/// The most outlines the cache holds. A full cache is cleared before the next insertion.
// Only glyphs off the atlas take outlines, so this bounds display-sized glyphs on screen at once.
// Clearing wholesale costs a handful of re-extractions in one frame.
const CAPACITY: usize = 512;

/// A cache of extracted outlines by key.
#[derive(Debug, Default)]
pub(crate) struct Outlines {
    held: Mutex<FxHashMap<OutlineKey, BezPath>>,
}

impl Outlines {
    /// Writes the outline of `key` of `font` at `coords` into `out`.
    ///
    /// Returns false, with `out` empty, when the font cannot be read or has no such glyph.
    pub(crate) fn write(
        &self,
        font: &FontData,
        key: &OutlineKey,
        coords: &[i16],
        out: &mut BezPath,
    ) -> bool {
        out.truncate(0);
        if let Some(held) = self.lock().get(key) {
            out.extend(held.iter());
            return true;
        }

        if !extract(font, key, coords, out) {
            out.truncate(0);
            return false;
        }

        let mut held = self.lock();
        if held.len() >= CAPACITY {
            held.clear();
        }
        held.insert(*key, out.clone());
        true
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, FxHashMap<OutlineKey, BezPath>> {
        self.held
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Draws the curves of one glyph into `out` in device pixels, y down, origin on the baseline.
///
/// Returns false when the font cannot be read, has no such glyph, or fails to draw it.
fn extract(font: &FontData, key: &OutlineKey, coords: &[i16], out: &mut BezPath) -> bool {
    let Ok(face) = FontRef::from_index(font.bytes(), font.index) else {
        return false;
    };
    let outlines = face.outline_glyphs();
    let Some(glyph) = outlines.get(GlyphId::new(key.glyph)) else {
        // A glyph the face has without an outline is blank.
        return key.glyph < u32::from(face.maxp().map_or(0, |maxp| maxp.num_glyphs()));
    };

    let location: Vec<F2Dot14> = coords.iter().copied().map(F2Dot14::from_bits).collect();
    let mut pen = Pen {
        path: out,
        shear: key.skew().to_radians().tan(),
    };
    glyph
        .draw(
            DrawSettings::unhinted(Size::new(key.size()), LocationRef::new(&location)),
            &mut pen,
        )
        .is_ok()
}
