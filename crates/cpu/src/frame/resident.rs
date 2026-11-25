//! The lowered chunks the renderer keeps, by revision.

use std::collections::HashMap;
use std::sync::Arc;

use iceblink::{ChunkNote, LoweredChunk};

/// The resident chunks, kept by applying each frame's notes.
#[derive(Debug, Default)]
pub(crate) struct Resident {
    chunks: HashMap<u64, Arc<LoweredChunk>>,
}

impl Resident {
    /// Applies `notes` in order.
    pub(crate) fn apply(&mut self, notes: &[ChunkNote]) {
        for note in notes {
            match note {
                ChunkNote::Inserted { revision, chunk } => {
                    self.chunks.insert(*revision, Arc::clone(chunk));
                }
                ChunkNote::Retired { revision } => {
                    self.chunks.remove(revision);
                }
                ChunkNote::ResetAll => self.chunks.clear(),
            }
        }
    }

    /// Returns the lowered chunk of `revision`, if it is resident.
    pub(crate) fn get(&self, revision: u64) -> Option<&LoweredChunk> {
        self.chunks.get(&revision).map(Arc::as_ref)
    }

    /// Returns how many chunks are resident.
    pub(crate) fn len(&self) -> usize {
        self.chunks.len()
    }
}
