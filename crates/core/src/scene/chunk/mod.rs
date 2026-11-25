//! A retained chunk: its recording, its offset and what is derived from them.

mod lowered;
mod orders;
mod runs;

pub(crate) use self::lowered::{
    Checked, ContentStamp, Lane, Lowered, LoweredItem, LoweredPath, LoweringKey, Part, Placement,
};
pub(crate) use self::orders::{LaneOrders, LocalOrders};
pub(crate) use self::runs::{RunInk, RunLowering};

use iceblink::RecordedChunk;
use smallvec::SmallVec;

use crate::scene::inks::ChunkInks;
use crate::scene::kind::ObjectRef;

/// A chunk as the store keeps it.
#[derive(Debug)]
pub(crate) struct ChunkRecord {
    /// The recording, moved in from the transaction that recorded it.
    pub(crate) recorded: RecordedChunk,
    pub(crate) offset: [f32; 2],
    pub(crate) derived: ChunkDerived,
}

/// What the store computes from a chunk after the commit that changed it.
#[derive(Debug, Default)]
pub(crate) struct ChunkDerived {
    /// Every object the chunk refers to, each once and in order.
    pub(crate) deps: Vec<ObjectRef>,
    /// The ink of each item and their union.
    pub(crate) inks: ChunkInks,
    /// The frame clip slot of each chunk-local clip.
    pub(crate) local_frames: SmallVec<[u32; 2]>,
    /// The inline colours of the recording, each once, which keep their frame paint slots until
    /// the recording is dropped.
    pub(crate) inline: SmallVec<[[u8; 4]; 2]>,
    /// The chunk as it was last lowered into frame kinds.
    pub(crate) lowered: Option<Lowered>,
    /// The local orders of the lowering's instances, when a frame drew it.
    pub(crate) orders: Option<LocalOrders>,
    /// Whether the chunk draws with a shader that reads the clock.
    pub(crate) reads_time: bool,
    /// Whether the chunk holds a backdrop whose filters read outside what it writes.
    pub(crate) reads_around: bool,
    /// The local ink of each glyph run, by its index among the chunk's runs.
    pub(crate) runs: Vec<Option<RunInk>>,
    /// The lowering each glyph run was measured on, by its index among the chunk's runs, until
    /// the chunk is next lowered.
    pub(crate) lowered_runs: Vec<Option<RunLowering>>,
    /// The bytes of the recording and lowerings when the record was last counted.
    pub(crate) counted: u64,
}

impl ChunkRecord {
    /// Returns a record of `recorded` that derives into `derived`, which must be empty.
    pub(crate) fn new(recorded: RecordedChunk, derived: ChunkDerived) -> Self {
        Self {
            recorded,
            offset: [0.0; 2],
            derived,
        }
    }
}

impl ChunkDerived {
    /// Empties the derived state, keeping the capacity of its lists.
    pub(crate) fn clear(&mut self) {
        self.deps.clear();
        self.inks.clear();
        self.local_frames.clear();
        self.inline.clear();
        self.lowered = None;
        self.orders = None;
        self.reads_time = false;
        self.reads_around = false;
        self.runs.clear();
        self.lowered_runs.clear();
        self.counted = 0;
    }
}
