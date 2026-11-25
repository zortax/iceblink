//! Mark payloads shared by resident chunks, and payloads gathered for one frame.

use std::ops::Range;

use iceblink::{Frame, FrameMarkPayload, FrameMarks, LoweredChunk};

use crate::buffer::chunks::lanes::{
    PAYLOAD_LANES, payload_counts, payload_element, payload_key, payload_slice,
};
use crate::buffer::chunks::{ChunkStore, payload_arena};
use crate::gpu::device::Gpu;
use crate::upload::UploadBelt;

/// A payload held on the device while any resident chunk holds it.
#[derive(Debug)]
pub(super) struct Shared {
    pub(super) payload: FrameMarkPayload,
    /// The range of each payload kind, once uploaded.
    pub(super) ranges: [Option<Range<u32>>; PAYLOAD_LANES],
    /// How many resident chunks hold the payload.
    pub(super) holders: u32,
}

impl ChunkStore {
    /// Takes a hold on each payload of `chunk`, uploads the ones nothing held yet, and returns
    /// their keys.
    pub(super) fn hold_payloads(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        chunk: &LoweredChunk,
    ) -> Box<[(usize, usize)]> {
        let mut payloads = Vec::with_capacity(chunk.mark_payloads.len());
        let mut fresh = Vec::new();
        let mut totals = [0u32; PAYLOAD_LANES];
        for payload in &chunk.mark_payloads {
            let key = payload_key(payload);
            payloads.push(key);
            if let Some(shared) = self.shared.get_mut(&key) {
                shared.holders += 1;
                continue;
            }
            self.shared.insert(
                key,
                Shared {
                    payload: payload.clone(),
                    ranges: Default::default(),
                    holders: 1,
                },
            );
            fresh.push(key);
            for (kind, count) in payload_counts(payload).into_iter().enumerate() {
                totals[kind] += count;
            }
        }
        // The payloads no resident chunk holds yet share one range per kind and one write.
        for (kind, &total) in totals.iter().enumerate() {
            if total == 0 {
                continue;
            }
            let arena = payload_arena(kind);
            let range = self.alloc(gpu, belt, encoder, arena, total);
            let mut bytes = Vec::with_capacity(total as usize * payload_element(kind) as usize);
            let mut start = range.start;
            for key in &fresh {
                let shared = self.shared.get_mut(key).expect("inserted above");
                let slice = payload_slice(&shared.payload, kind);
                let count = (slice.len() / payload_element(kind) as usize) as u32;
                if count == 0 {
                    continue;
                }
                bytes.extend_from_slice(slice);
                shared.ranges[kind] = Some(start..start + count);
                start += count;
            }
            self.uploaded += self.arenas[arena].upload(gpu, belt, encoder, range.start, &bytes);
        }
        payloads.into_boxed_slice()
    }

    /// Drops one hold on the payload `key`, and retires its ranges once nothing holds it.
    pub(super) fn drop_hold(&mut self, key: (usize, usize)) {
        let Some(shared) = self.shared.get_mut(&key) else {
            return;
        };
        shared.holders = shared.holders.saturating_sub(1);
        if shared.holders > 0 {
            return;
        }
        let shared = self.shared.remove(&key).expect("just found");
        for (kind, range) in shared.ranges.into_iter().enumerate() {
            if let Some(range) = range {
                self.ledger.retire(payload_arena(kind), range);
            }
        }
    }

    /// Resolves the payload ranges of every marks instance of the frame, and gathers the payloads
    /// no resident chunk holds into ranges of their own for this frame.
    pub(super) fn resolve_payloads(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        frame: &Frame,
    ) {
        self.mark_payload.clear();
        if frame.lanes.marks.sources.is_empty() {
            return;
        }
        // Counted first, so the gathered payloads take one range per kind.
        let transient = self.transient_counts(frame);
        let mut ranges: [Range<u32>; PAYLOAD_LANES] = core::array::from_fn(|_| 0..0);
        for (kind, range) in ranges.iter_mut().enumerate() {
            if transient[kind] > 0 {
                *range = self.alloc(gpu, belt, encoder, payload_arena(kind), transient[kind]);
            }
        }
        let mut gathered: [Vec<u8>; PAYLOAD_LANES] = Default::default();
        let mut placed = [0u32; PAYLOAD_LANES];
        let mut found = std::mem::take(&mut self.mark_payload);
        for source in frame.lanes.marks.instances() {
            let Some(payload) = self.payload_of(frame, source) else {
                found.push(core::array::from_fn(|_| 0..0));
                continue;
            };
            let counts = payload_counts(payload);
            let at: [u32; PAYLOAD_LANES] = match self.shared.get(&payload_key(payload)) {
                Some(shared) => core::array::from_fn(|kind| {
                    shared.ranges[kind].as_ref().map_or(0, |range| range.start)
                }),
                None => {
                    let at = core::array::from_fn(|kind| ranges[kind].start + placed[kind]);
                    for kind in 0..PAYLOAD_LANES {
                        gathered[kind].extend_from_slice(payload_slice(payload, kind));
                        placed[kind] += counts[kind];
                    }
                    at
                }
            };
            found.push(core::array::from_fn(|kind| {
                at[kind]..at[kind] + counts[kind]
            }));
        }
        self.mark_payload = found;
        for (kind, bytes) in gathered.iter().enumerate() {
            let arena = payload_arena(kind);
            self.uploaded +=
                self.arenas[arena].upload(gpu, belt, encoder, ranges[kind].start, bytes);
            self.ledger.retire(arena, ranges[kind].clone());
        }
    }

    /// Returns the record counts, per kind, of the payloads the frame's marks draw that no
    /// resident chunk holds.
    fn transient_counts(&self, frame: &Frame) -> [u32; PAYLOAD_LANES] {
        let mut transient = [0u32; PAYLOAD_LANES];
        for source in frame.lanes.marks.instances() {
            if let Some(payload) = self.payload_of(frame, source)
                && !self.shared.contains_key(&payload_key(payload))
            {
                for (kind, count) in payload_counts(payload).into_iter().enumerate() {
                    transient[kind] += count;
                }
            }
        }
        transient
    }

    /// Returns the payload a marks `source` draws, from the frame or from its chunk.
    pub(super) fn payload_of<'a>(
        &'a self,
        frame: &'a Frame,
        source: iceblink::ItemSource,
    ) -> Option<&'a FrameMarkPayload> {
        if source.is_transient() {
            let marks: &FrameMarks = frame.lanes.marks.items.get(source.index as usize)?;
            return frame.lanes.mark_payloads.get(marks.marks.payload as usize);
        }
        let drawn = frame.drawn.get(source.chunk as usize)?;
        let chunk = &self.residence.get(&drawn.revision)?.chunk;
        let marks = chunk.marks.get(source.index as usize)?;
        chunk.mark_payloads.get(marks.marks.payload as usize)
    }
}
