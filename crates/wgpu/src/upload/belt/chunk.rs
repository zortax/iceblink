//! One staging buffer of a belt.

/// One mapped staging buffer.
#[derive(Debug)]
pub(super) struct Chunk {
    pub(super) buffer: wgpu::Buffer,
    pub(super) size: u64,
    /// Where the next allocation in the chunk starts.
    pub(super) offset: u64,
    /// The frame that last wrote into the chunk.
    pub(super) last_used: u64,
    /// Whether the chunk is dropped after its copies, instead of being recycled.
    // The pieces of a large write get exact chunks that are dropped with their frame, so one
    // large transfer does not keep megabytes of mapped memory.
    pub(super) one_shot: bool,
}

impl Chunk {
    /// Returns where an aligned allocation of `size` would begin, or `None` if it does not fit.
    pub(super) fn allocation(&self, size: u64, alignment: u64) -> Option<u64> {
        let offset = self
            .offset
            .next_multiple_of(alignment.max(wgpu::MAP_ALIGNMENT));
        (offset + size <= self.size).then_some(offset)
    }
}
