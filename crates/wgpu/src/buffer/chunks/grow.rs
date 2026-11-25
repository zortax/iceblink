//! Ranges taken from the arenas, and arenas that grow to hold them.

use std::ops::Range;

use crate::buffer::chunks::lanes::{LANES, LaneKind, chunk_bytes, payload_slice};
use crate::buffer::chunks::{ChunkStore, payload_arena};
use crate::gpu::device::Gpu;
use crate::upload::UploadBelt;

impl ChunkStore {
    /// Returns a range of `count` records of `arena`, which grows when it has no room.
    ///
    /// The range lies past the capacity when the arena cannot grow to hold it.
    pub(super) fn alloc(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        arena: usize,
        count: u32,
    ) -> Range<u32> {
        if let Some(range) = self.arenas[arena].alloc(count) {
            return range;
        }
        // An arena at the largest texture of the device cannot grow.
        if self.arenas[arena].capacity >= self.arenas[arena].limit(gpu) {
            return self.arenas[arena].past(count);
        }
        self.uploaded += self.grow(gpu, belt, encoder, arena, count);
        let arena = &mut self.arenas[arena];
        match arena.alloc(count) {
            Some(range) => range,
            None => arena.past(count),
        }
    }

    /// Replaces arena `lane` with one at least twice as large and large enough for `incoming`
    /// more records, uploads what it held again, and returns the bytes staged.
    pub(super) fn grow(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        lane: usize,
        incoming: u32,
    ) -> u64 {
        let length = |range: &Option<Range<u32>>| range.as_ref().map_or(0, |r| r.end - r.start);
        let live: u32 = match lane.checked_sub(LANES) {
            None => self
                .residence
                .values()
                .filter_map(|resident| resident.ranges.as_ref())
                .map(|ranges| length(&ranges[lane]))
                .sum(),
            Some(kind) => self
                .shared
                .values()
                .map(|shared| length(&shared.ranges[kind]))
                .sum(),
        };
        let needed = (live + incoming)
            .saturating_mul(2)
            .max(self.arenas[lane].capacity.saturating_mul(2));
        self.arenas[lane].reset_with_capacity(gpu, needed);
        // Pending ranges of every lane may name the replaced texture. Forgetting them all costs
        // the other lanes those ranges until they grow themselves.
        self.ledger.forget();
        match lane.checked_sub(LANES) {
            Some(kind) => self.settle_payloads(gpu, belt, encoder, kind),
            None => self.settle_chunks(gpu, belt, encoder, lane),
        }
    }

    /// Uploads every held payload of `kind` into its arena again, and returns the bytes staged.
    fn settle_payloads(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        kind: usize,
    ) -> u64 {
        let mut uploaded = 0;
        let arena = &mut self.arenas[payload_arena(kind)];
        for shared in self.shared.values_mut() {
            let bytes = payload_slice(&shared.payload, kind);
            let count = (bytes.len() as u32) / arena.element;
            // A payload being inserted has no range yet, and takes one after the growth.
            if count == 0 || shared.ranges[kind].is_none() {
                continue;
            }
            let range = arena.alloc(count).unwrap_or_else(|| arena.past(count));
            uploaded += arena.upload(gpu, belt, encoder, range.start, bytes);
            shared.ranges[kind] = Some(range);
        }
        uploaded
    }

    /// Uploads the `lane` arrays of every resident chunk into its arena again, and returns the
    /// bytes staged.
    fn settle_chunks(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        lane: usize,
    ) -> u64 {
        let mut uploaded = 0;
        let arena = &mut self.arenas[lane];
        for resident in self.residence.values_mut() {
            let Some(ranges) = resident.ranges.as_mut() else {
                continue;
            };
            let bytes = chunk_bytes(&resident.chunk, LaneKind::ALL[lane]);
            let count = (bytes.len() as u32) / arena.element;
            if count == 0 {
                continue;
            }
            let range = arena.alloc(count).unwrap_or_else(|| arena.past(count));
            uploaded += arena.upload(gpu, belt, encoder, range.start, bytes);
            ranges[lane] = Some(range);
        }
        uploaded
    }
}
