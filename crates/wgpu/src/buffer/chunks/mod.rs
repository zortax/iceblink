//! Lowered chunks kept on the device, so a frame that draws them again uploads nothing.
//!
//! Each lane has an arena, and so does each mark payload kind. An inserted chunk's arrays are
//! uploaded into ranges of the arenas once. Every frame then resolves its lanes' sources into a
//! draw order of arena slots and offsets. A resident source refers to its chunk's range, and a
//! transient one is gathered into a range of its own for that frame alone. A payload shared by
//! several chunks is uploaded once and held while any of them is resident.
//!
//! A range is returned to its arena only once the submission that last read it has completed.

mod arena;
mod grow;
mod lanes;
mod ledger;
mod payloads;
mod resolve;

use std::ops::Range;
use std::sync::Arc;

use iceblink::{ChunkNote, Frame, FrameMarks, LoweredChunk};
use rustc_hash::FxHashMap;

use crate::buffer::chunks::arena::Arena;
use crate::buffer::chunks::lanes::{LaneKind, chunk_bytes, payload_element, payload_label};
use crate::buffer::chunks::ledger::RetireLedger;
use crate::buffer::chunks::payloads::Shared;
use crate::gpu::device::Gpu;
use crate::upload::UploadBelt;

pub(crate) use self::lanes::{CELLS, LANES, MARKS_LANE, PAYLOAD_LANES, SHADER_QUADS_LANE};

/// Returns the arena of mark payload kind `kind`.
const fn payload_arena(kind: usize) -> usize {
    LANES + kind
}

/// One entry of a lane's draw order: an arena slot and the offset it is drawn moved by.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct OrderEntry {
    pub(crate) slot: u32,
    pub(crate) shift: [f32; 2],
}

/// A resident chunk and the ranges that hold its arrays.
#[derive(Debug)]
struct Resident {
    chunk: Arc<LoweredChunk>,
    /// The range of each lane's array, or `None` while the arrays are not on the device.
    ranges: Option<[Option<Range<u32>>; LANES]>,
    /// The keys of the payloads the chunk holds.
    payloads: Box<[(usize, usize)]>,
}

/// Resident chunks and the arenas that hold them.
#[derive(Debug)]
pub(crate) struct ChunkStore {
    arenas: Vec<Arena>,
    residence: FxHashMap<u64, Resident>,
    /// The payloads held on the device, by key.
    shared: FxHashMap<(usize, usize), Shared>,
    /// The payload holds that retired chunks drop once the notes are applied.
    released: Vec<(usize, usize)>,
    ledger: RetireLedger,
    /// Per lane, the frame's draw order.
    resolved: [Vec<OrderEntry>; LANES],
    /// The transient instances of the lane being resolved.
    gathered: Vec<u8>,
    /// Per instance of the frame's marks lane, its range of each payload kind.
    mark_payload: Vec<[Range<u32>; PAYLOAD_LANES]>,
    /// The bytes staged since the last `take_uploaded`.
    uploaded: u64,
}

impl ChunkStore {
    pub(crate) fn new(gpu: &Gpu) -> Self {
        let mut arenas: Vec<Arena> = LaneKind::ALL
            .iter()
            .map(|lane| Arena::new(gpu, lane.label(), lane.element()))
            .collect();
        arenas.extend(
            (0..PAYLOAD_LANES)
                .map(|kind| Arena::new(gpu, payload_label(kind), payload_element(kind))),
        );
        Self {
            arenas,
            residence: FxHashMap::default(),
            shared: FxHashMap::default(),
            released: Vec::new(),
            ledger: RetireLedger::new(),
            resolved: Default::default(),
            gathered: Vec::new(),
            mark_payload: Vec::new(),
            uploaded: 0,
        }
    }

    pub(crate) fn binding(&self, lane: usize) -> wgpu::BindingResource<'_> {
        wgpu::BindingResource::TextureView(&self.arenas[lane].view)
    }

    /// Returns a number that changes whenever the arena of `lane` gets a new texture.
    pub(crate) fn generation(&self, lane: usize) -> u64 {
        self.arenas[lane].generation
    }

    pub(crate) fn payload_binding(&self, kind: usize) -> wgpu::BindingResource<'_> {
        wgpu::BindingResource::TextureView(&self.arenas[payload_arena(kind)].view)
    }

    /// Returns a number that changes whenever the arena of payload kind `kind` gets a new
    /// texture.
    pub(crate) fn payload_generation(&self, kind: usize) -> u64 {
        self.arenas[payload_arena(kind)].generation
    }

    /// Returns the payload ranges of the marks instance at `position` of the marks lane.
    pub(crate) fn mark_payload(&self, position: usize) -> [Range<u32>; PAYLOAD_LANES] {
        self.mark_payload
            .get(position)
            .cloned()
            .unwrap_or_else(|| core::array::from_fn(|_| 0..0))
    }

    /// Returns the bytes the arenas hold.
    pub(crate) fn bytes(&self) -> u64 {
        self.arenas
            .iter()
            .map(|arena| u64::from(arena.capacity) * u64::from(arena.stride))
            .sum()
    }

    /// Returns how many chunks are resident.
    pub(crate) fn resident(&self) -> usize {
        self.residence.len()
    }

    /// Returns the bytes uploaded for chunks and their payloads since the last call.
    pub(crate) fn take_uploaded(&mut self) -> u64 {
        std::mem::take(&mut self.uploaded)
    }

    /// Applies a frame's chunk notes in order.
    ///
    /// The notes can span frames that were never drawn, so a revision can be inserted and retired
    /// by one list. Payload holds drop after every insertion, so a payload that passes from one
    /// revision to the next stays on the device.
    pub(crate) fn apply_notes(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        notes: &[ChunkNote],
    ) {
        self.ledger.reclaim(&mut self.arenas);
        self.released.clear();
        for note in notes {
            match note {
                ChunkNote::Inserted { revision, chunk } => {
                    if !self.residence.contains_key(revision) {
                        let resident = self.insert(gpu, belt, encoder, chunk);
                        self.residence.insert(*revision, resident);
                    }
                }
                ChunkNote::Retired { revision } => {
                    if let Some(resident) = self.residence.remove(revision) {
                        self.retire(resident);
                    }
                }
                ChunkNote::ResetAll => {
                    let all: Vec<u64> = self.residence.keys().copied().collect();
                    for revision in all {
                        if let Some(resident) = self.residence.remove(&revision) {
                            self.retire(resident);
                        }
                    }
                }
            }
        }
        let released = core::mem::take(&mut self.released);
        for key in &released {
            self.drop_hold(*key);
        }
        self.released = released;
    }

    /// Retires the ranges of `resident`, and queues its payload holds to drop.
    fn retire(&mut self, resident: Resident) {
        if let Some(ranges) = resident.ranges {
            for (lane, range) in ranges.into_iter().enumerate() {
                if let Some(range) = range {
                    self.ledger.retire(lane, range);
                }
            }
        }
        self.released.extend_from_slice(&resident.payloads);
    }

    /// Uploads the arrays of `chunk` and takes holds on its payloads.
    fn insert(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        chunk: &Arc<LoweredChunk>,
    ) -> Resident {
        let ranges = self.upload_lanes(gpu, belt, encoder, chunk);
        let payloads = self.hold_payloads(gpu, belt, encoder, chunk);
        Resident {
            chunk: Arc::clone(chunk),
            ranges: Some(ranges),
            payloads,
        }
    }

    /// Allocates and stages a range for each lane array of `chunk`, and returns the ranges.
    pub(super) fn upload_lanes(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        chunk: &LoweredChunk,
    ) -> [Option<Range<u32>>; LANES] {
        let mut ranges: [Option<Range<u32>>; LANES] = Default::default();
        for (lane, held) in ranges.iter_mut().enumerate() {
            let bytes = chunk_bytes(chunk, LaneKind::ALL[lane]);
            let count = (bytes.len() / self.arenas[lane].element as usize) as u32;
            if count == 0 {
                continue;
            }
            let range = self.alloc(gpu, belt, encoder, lane, count);
            self.uploaded += self.arenas[lane].upload(gpu, belt, encoder, range.start, bytes);
            *held = Some(range);
        }
        ranges
    }

    /// Returns the frame's draw order of `lane`.
    pub(crate) fn resolved_order(&self, lane: usize) -> &[OrderEntry] {
        &self.resolved[lane]
    }

    /// Ties the ranges retired since the last submission to the submission just made.
    pub(crate) fn submitted(&mut self, gpu: &Gpu) {
        self.ledger.submitted(gpu);
    }

    /// Frees every arena, and returns the bytes freed.
    ///
    /// The chunks stay resident, and their arrays and payloads are uploaded again when they are
    /// next drawn.
    pub(crate) fn release(&mut self, gpu: &Gpu) -> u64 {
        let before = self.bytes();
        for resident in self.residence.values_mut() {
            resident.ranges = None;
        }
        for shared in self.shared.values_mut() {
            shared.ranges = Default::default();
        }
        self.ledger.forget();
        for arena in &mut self.arenas {
            arena.reset_with_capacity(gpu, 0);
        }
        self.shared.clear();
        for resident in self.residence.values_mut() {
            resident.payloads = Box::new([]);
        }
        before.saturating_sub(self.bytes())
    }

    /// Returns the marks instance at `position` of the frame's marks lane, unmoved.
    pub(crate) fn marks_item(&self, frame: &Frame, position: usize) -> Option<FrameMarks> {
        let source = frame.lanes.marks.instances().nth(position)?;
        if source.is_transient() {
            return frame.lanes.marks.items.get(source.index as usize).copied();
        }
        let drawn = frame.drawn.get(source.chunk as usize)?;
        let chunk = &self.residence.get(&drawn.revision)?.chunk;
        chunk.marks.get(source.index as usize).copied()
    }
}
