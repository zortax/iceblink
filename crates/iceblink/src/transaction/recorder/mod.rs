//! Recording the items of a chunk.

mod hint;
mod items;
mod lists;
mod refs;

use crate::transaction::RecordedChunk;

/// Appends items to a recorded chunk.
///
/// Each item push returns the item's index in the chunk, counting every kind from zero in record
/// order.
///
/// The reference methods turn an id into the slot an item stores, and add the id to the chunk's
/// `refs`, so a commit rejects the chunk if the id is stale.
#[derive(Debug)]
pub struct ChunkRecorder<'a> {
    chunk: &'a mut RecordedChunk,
}

impl<'a> ChunkRecorder<'a> {
    pub(crate) fn new(chunk: &'a mut RecordedChunk) -> Self {
        Self { chunk }
    }
}

/// Pushes `value` and returns its index in `list`.
fn index_of_push<T>(list: &mut Vec<T>, value: T) -> u32 {
    let index = list.len() as u32;
    list.push(value);
    index
}
