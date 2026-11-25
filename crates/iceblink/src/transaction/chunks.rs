//! Transaction commands on chunks.

use crate::chunk::{ChunkKey, ChunkProps};
use crate::transaction::{ChunkRecorder, Command, RecordedChunk, Transaction};

impl Transaction {
    /// Records the chunk `key` with default property state `props`, replacing the whole chunk.
    ///
    /// Recording a chunk resets its offset to zero. A key recorded twice in one transaction keeps
    /// the second recording.
    pub fn record(&mut self, key: ChunkKey, props: ChunkProps) -> ChunkRecorder<'_> {
        let chunk = self.empty_chunk(key, props);
        self.record_chunk(chunk);
        let index = self.chunks.len() - 1;
        ChunkRecorder::new(&mut self.chunks[index])
    }

    /// Returns an empty chunk with key `key` and default property state `props`.
    ///
    /// The chunk reuses the buffers of a recycled chunk if one is available. It is not part of the
    /// transaction until it is passed to `record_chunk`, and the transaction takes other commands
    /// meanwhile.
    pub fn empty_chunk(&mut self, key: ChunkKey, props: ChunkProps) -> RecordedChunk {
        match self.spare.pop() {
            Some(mut chunk) => {
                chunk.key = key;
                chunk.props = props;
                chunk
            }
            None => RecordedChunk::new(key, props),
        }
    }

    /// Records `chunk` under its key, replacing the whole chunk, with the rules of `record`.
    pub fn record_chunk(&mut self, chunk: RecordedChunk) {
        let index = self.chunks.len();
        self.chunks.push(chunk);
        self.commands.push(Command::Record(index as u32));
    }

    /// Writes the default property state of the chunk `key`.
    pub fn set_props(&mut self, key: ChunkKey, props: ChunkProps) {
        self.commands.push(Command::SetProps(key, props));
    }

    /// Translates the whole chunk `key` by `offset`, in each item's own local units.
    ///
    /// The paint geometry and the chunk-local clips of the chunk move with it. The offset replaces
    /// the previous one and persists until the next `set_offset` or the next recording of the
    /// chunk.
    pub fn set_offset(&mut self, key: ChunkKey, offset: [f32; 2]) {
        self.commands.push(Command::SetOffset(key, offset));
    }

    /// Removes the chunk `key`.
    pub fn remove_chunk(&mut self, key: ChunkKey) {
        self.commands.push(Command::RemoveChunk(key));
    }
}
