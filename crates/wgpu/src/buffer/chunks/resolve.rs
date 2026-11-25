//! Resolving a frame's lane sources into draw orders of arena slots.

use std::sync::Arc;

use iceblink::{DrawnChunk, Frame, ItemSource};

use crate::buffer::chunks::lanes::{LaneKind, chunk_bytes, lane_items, lane_sources};
use crate::buffer::chunks::{ChunkStore, OrderEntry};
use crate::gpu::device::Gpu;
use crate::upload::UploadBelt;

impl ChunkStore {
    /// Resolves the frame's lanes into draw orders, gathering every transient instance, and records
    /// the copies of everything uploaded into the arenas since the last call.
    ///
    /// A resident chunk whose arrays were released is uploaded again first. A source that refers
    /// to a revision the store does not hold draws a zeroed instance, which paints nothing.
    pub(crate) fn resolve(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        frame: &Frame,
    ) {
        self.restore(gpu, belt, encoder, &frame.drawn);
        for lane in LaneKind::ALL {
            self.resolve_lane(gpu, belt, encoder, frame, lane);
        }
        self.resolve_payloads(gpu, belt, encoder, frame);
        for arena in &mut self.arenas {
            arena.flush(gpu, belt, encoder);
        }
    }

    /// Uploads again the chunks of `drawn` whose arrays were released.
    pub(super) fn restore(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        drawn: &[DrawnChunk],
    ) {
        for chunk in drawn {
            let Some(resident) = self.residence.get(&chunk.revision) else {
                continue;
            };
            if resident.ranges.is_some() {
                continue;
            }
            let held = Arc::clone(&resident.chunk);
            let ranges = self.upload_lanes(gpu, belt, encoder, &held);
            let payloads = self.hold_payloads(gpu, belt, encoder, &held);
            if let Some(resident) = self.residence.get_mut(&chunk.revision) {
                resident.ranges = Some(ranges);
                resident.payloads = payloads;
            }
        }
    }

    /// Returns the order entry of instance `index` of `lane` in drawn chunk `chunk`, or `None`
    /// if that instance is not on the device.
    pub(super) fn slot_of(
        &self,
        drawn: &[DrawnChunk],
        lane: usize,
        chunk: u32,
        index: u32,
    ) -> Option<OrderEntry> {
        let drawn = drawn.get(chunk as usize)?;
        let range = self.residence.get(&drawn.revision)?.ranges.as_ref()?[lane].as_ref()?;
        let slot = range
            .start
            .checked_add(index)
            .filter(|slot| *slot < range.end)?;
        Some(OrderEntry {
            slot,
            shift: drawn.offset,
        })
    }

    /// Returns the order entry of the first instance of a resident run, when the whole run lies
    /// in its chunk's range.
    fn run_of(&self, drawn: &[DrawnChunk], lane: usize, run: ItemSource) -> Option<OrderEntry> {
        if run.is_transient() || run.len == 0 {
            return None;
        }
        self.slot_of(drawn, lane, run.chunk, run.index + run.len - 1)?;
        self.slot_of(drawn, lane, run.chunk, run.index)
    }

    /// Resolves `lane` of the frame into its draw order, and gathers its transient instances
    /// into a range of their own for this frame.
    pub(super) fn resolve_lane(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        frame: &Frame,
        lane: LaneKind,
    ) {
        let at = lane.index();
        let sources = lane_sources(&frame.lanes, lane);
        let items = lane_items(&frame.lanes, lane);
        let element = self.arenas[at].element as usize;
        let mut resolved = std::mem::take(&mut self.resolved[at]);
        resolved.clear();
        let mut gathered = std::mem::take(&mut self.gathered);
        gathered.clear();

        let transients = sources
            .iter()
            .filter(|source| self.run_of(&frame.drawn, at, **source).is_none())
            .map(|source| source.len)
            .sum::<u32>();
        let range = if transients > 0 {
            self.alloc(gpu, belt, encoder, at, transients)
        } else {
            0..0
        };

        let mut placed = 0;
        for source in sources {
            if let Some(first) = self.run_of(&frame.drawn, at, *source) {
                resolved.extend((0..source.len).map(|offset| OrderEntry {
                    slot: first.slot + offset,
                    shift: first.shift,
                }));
                continue;
            }
            for index in source.indices() {
                let one = ItemSource {
                    index,
                    len: 1,
                    ..*source
                };
                let (bytes, shift) = self.gather_source(frame, lane, items, one, element);
                match bytes {
                    Some(bytes) => gathered.extend_from_slice(bytes),
                    None => gathered.resize(gathered.len() + element, 0),
                }
                resolved.push(OrderEntry {
                    slot: range.start + placed,
                    shift,
                });
                placed += 1;
            }
        }
        if !gathered.is_empty() {
            self.uploaded += self.arenas[at].upload(gpu, belt, encoder, range.start, &gathered);
        }
        self.gathered = gathered;
        // A frame's gathered instances are reclaimable once its submission completes.
        self.ledger.retire(at, range);
        self.resolved[at] = resolved;
    }

    /// Returns the bytes of one instance `source`, from the frame or from its resident chunk, and
    /// the offset it is drawn moved by.
    ///
    /// The bytes are `None` for a source the frame or the store does not hold.
    pub(super) fn gather_source<'a>(
        &'a self,
        frame: &'a Frame,
        lane: LaneKind,
        items: &'a [u8],
        source: iceblink::ItemSource,
        element: usize,
    ) -> (Option<&'a [u8]>, [f32; 2]) {
        if source.is_transient() {
            let at = source.index as usize * element;
            return (items.get(at..at + element), [0.0, 0.0]);
        }
        let Some(drawn) = frame.drawn.get(source.chunk as usize) else {
            return (None, [0.0, 0.0]);
        };
        let Some(resident) = self.residence.get(&drawn.revision) else {
            return (None, [0.0, 0.0]);
        };
        let bytes = chunk_bytes(&resident.chunk, lane);
        let at = source.index as usize * element;
        (bytes.get(at..at + element), drawn.offset)
    }
}
