//! The space table: world matrices, their encodings, and the clips a moved space resolves again.

mod encode;

use iceblink::Tables;

use crate::bind::tables::{FrameTables, GpuSpatial};

pub(crate) use self::encode::{axis_aligned, translation};

impl FrameTables {
    /// Applies the space delta, and queues the clips whose records a changed matrix affects.
    pub(super) fn apply_spaces(&mut self, tables: &Tables) {
        let delta = &tables.spaces;
        if delta.is_unchanged() {
            return;
        }
        delta.apply(&mut self.spaces);
        if delta.replace {
            self.gpu.spatial = self.spaces.iter().map(GpuSpatial::of).collect();
            self.translating = self
                .spaces
                .iter()
                .map(|matrix| translation(matrix).is_some())
                .collect();
            self.dirty.spatial.mark_all();
            for slot in 0..self.clips.len() as u32 {
                self.enqueue(slot);
            }
            return;
        }
        self.gpu
            .spatial
            .resize(self.spaces.len(), GpuSpatial::default());
        self.translating.resize(self.spaces.len(), true);
        for &(slot, _) in &delta.changed {
            let at = slot as usize;
            let matrix = self.spaces[at];
            self.gpu.spatial[at] = GpuSpatial::of(&matrix);
            self.dirty.spatial.push(slot);

            let now = translation(&matrix).is_some();
            let was = std::mem::replace(&mut self.translating[at], now);
            let mut queued = Vec::new();
            self.deps.members(slot, false, |clip| queued.push(clip));
            if was != now {
                self.deps.members(slot, true, |clip| queued.push(clip));
            }
            for clip in queued {
                self.enqueue(clip);
            }
        }
    }
}
