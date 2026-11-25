//! The paint table: frame paints encoded into records and ranges of the stop table.

mod encode;

use iceblink::{FramePaint, Tables};

use crate::bind::tables::{FrameTables, GpuPaint, GpuStop};

impl FrameTables {
    /// Applies the paint delta, quantises the changed paints and encodes them.
    pub(super) fn apply_paints(&mut self, tables: &Tables) {
        let delta = &tables.paints;
        if delta.is_unchanged() {
            return;
        }
        delta.apply(&mut self.paints);
        if delta.replace {
            self.paints.iter_mut().for_each(FramePaint::quantize);
            self.rebuild_paints();
            return;
        }
        self.gpu
            .paints
            .resize(self.paints.len(), GpuPaint::default());
        self.stop_capacity.resize(self.paints.len(), 0);
        for &(slot, _) in &delta.changed {
            self.paints[slot as usize].quantize();
            self.encode_paint(slot);
        }
        if self.stop_garbage as usize > self.gpu.stops.len() / 2 + 1024 {
            self.rebuild_paints();
        }
    }

    /// Encodes every paint into new paint and stop tables.
    fn rebuild_paints(&mut self) {
        self.gpu.paints.clear();
        self.gpu.stops.clear();
        self.stop_capacity.clear();
        self.stop_garbage = 0;
        self.gpu
            .paints
            .resize(self.paints.len(), GpuPaint::default());
        self.stop_capacity.resize(self.paints.len(), 0);
        for slot in 0..self.paints.len() as u32 {
            self.encode_paint(slot);
        }
        self.dirty.paints.mark_all();
        self.dirty.stops.mark_all();
    }

    /// Encodes paint `slot`, and marks the records that changed.
    ///
    /// A paint rewrites its stops in place while they fit, and otherwise takes a range at the end.
    fn encode_paint(&mut self, slot: u32) {
        let at = slot as usize;
        let mut record = encode::encode(&self.paints[at], &mut self.scratch);
        let count = self.scratch.len() as u32;
        let held = self.gpu.paints[at];
        let capacity = self.stop_capacity[at];
        let start = if count <= capacity {
            held.stop_start
        } else {
            self.stop_garbage += capacity;
            self.stop_capacity[at] = count;
            let start = self.gpu.stops.len() as u32;
            self.gpu
                .stops
                .resize(start as usize + count as usize, GpuStop::default());
            start
        };
        record.stop_start = start;
        record.stop_count = count;
        for (offset, stop) in self.scratch.iter().enumerate() {
            let index = start as usize + offset;
            if self.gpu.stops[index] != *stop {
                self.gpu.stops[index] = *stop;
                self.dirty.stops.push(index as u32);
            }
        }
        if held != record {
            self.gpu.paints[at] = record;
            self.dirty.paints.push(slot);
        }
    }
}
