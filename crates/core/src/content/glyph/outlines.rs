//! Glyph outlines, shared so that a path keeps its address across lowerings.

use std::sync::Arc;

use iceblink::{FontData, GlyphRaster, OutlineKey};
use kurbo::BezPath;
use rustc_hash::FxHashMap;

/// The outlines kept after a frame that used none of them.
const KEPT_OUTLINES: usize = 4_096;

/// How many frames an unused outline survives once the cache holds more than
/// [`KEPT_OUTLINES`].
const IDLE_FRAMES: u64 = 8;

/// One cached outline.
#[derive(Debug)]
struct Cached {
    /// The curves, or `None` when the font has no such glyph.
    path: Option<Arc<BezPath>>,
    /// The frame that last used the outline.
    frame: u64,
}

/// The outlines written so far, by key.
#[derive(Debug, Default)]
pub(crate) struct OutlineCache {
    outlines: FxHashMap<OutlineKey, Cached>,
    frame: u64,
    written: u64,
}

/// The answer for one outline.
#[derive(Clone, Debug)]
pub(crate) enum Outline {
    /// The curves, possibly empty for a blank glyph.
    Curves(Arc<BezPath>),
    /// The font has no such glyph.
    Missing,
}

impl OutlineCache {
    /// Returns the outline of `key`, asking `raster` for it on a miss.
    pub(crate) fn outline(
        &mut self,
        raster: &dyn GlyphRaster,
        font: &FontData,
        key: &OutlineKey,
        coords: &[i16],
    ) -> Outline {
        let frame = self.frame;
        let mut wrote = false;
        let cached = self.outlines.entry(*key).or_insert_with(|| {
            wrote = true;
            let mut path = BezPath::new();
            let found = raster.outline(font, key, coords, &mut path);
            Cached {
                path: found.then(|| Arc::new(path)),
                frame,
            }
        });
        cached.frame = frame;
        self.written += u64::from(wrote);

        match &cached.path {
            Some(path) => Outline::Curves(Arc::clone(path)),
            None => Outline::Missing,
        }
    }

    /// Starts a frame, dropping outlines idle for [`IDLE_FRAMES`] when more than
    /// [`KEPT_OUTLINES`] are held.
    pub(crate) fn begin_frame(&mut self) {
        self.frame += 1;
        if self.outlines.len() > KEPT_OUTLINES {
            let frame = self.frame;
            self.outlines
                .retain(|_, cached| frame - cached.frame <= IDLE_FRAMES);
        }
    }

    /// Returns the outlines written since the cache was made.
    pub(crate) fn written(&self) -> u64 {
        self.written
    }

    /// Forgets every outline.
    pub(crate) fn clear(&mut self) {
        self.outlines.clear();
    }
}
