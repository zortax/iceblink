//! Sorted glyphs cut into batches of one font and one set of coordinates.

use std::sync::Arc;

use iceblink::{FontData, FontId, GlyphKey};
use rustc_hash::FxHashMap;

use crate::content::glyph::instances::Instances;

/// The number of batches cut per worker.
// More than one, so a worker that gets the expensive glyphs does not hold up the others. Few, so
// each batch prepares the hinting state of few fonts and sizes.
const BATCHES_PER_WORKER: usize = 2;

/// Glyphs of one font at one set of variation coordinates.
#[derive(Debug)]
pub(super) struct Batch {
    pub(super) font: FontData,
    pub(super) coords: Arc<[i16]>,
    pub(super) keys: Vec<GlyphKey>,
}

/// Cuts `keys`, sorted by font and instance, into batches.
///
/// A parallel cut aims at [`BATCHES_PER_WORKER`] batches per available core. A serial one makes
/// one batch per font and instance. Keys of a font missing from `fonts` are dropped.
pub(super) fn cut(
    keys: Vec<GlyphKey>,
    fonts: &FxHashMap<FontId, Option<FontData>>,
    instances: &Instances,
    parallel: bool,
) -> Vec<Batch> {
    let per_batch = if parallel {
        let batches = (super::workers::available() * BATCHES_PER_WORKER).max(1);
        keys.len().div_ceil(batches).max(1)
    } else {
        usize::MAX
    };

    let mut batches: Vec<Batch> = Vec::new();
    for key in keys {
        let same = batches.last().is_some_and(|batch| {
            let first = batch.keys[0];
            first.font == key.font && first.instance == key.instance && batch.keys.len() < per_batch
        });

        if same {
            if let Some(batch) = batches.last_mut() {
                batch.keys.push(key);
            }
            continue;
        }

        let Some(Some(font)) = fonts.get(&key.font) else {
            continue;
        };
        batches.push(Batch {
            font: font.clone(),
            coords: instances.coords(key.instance),
            keys: vec![key],
        });
    }
    batches
}
