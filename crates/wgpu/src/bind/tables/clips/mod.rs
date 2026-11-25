//! The clip table: clip entries resolved into records and ranges of the link table.

mod resolve;

use iceblink::Tables;

use crate::bind::tables::{FrameTables, GpuClip, GpuClipLink, MaskRequest};

pub(crate) use self::resolve::{MaskLink, Membership, resolve};

impl FrameTables {
    /// Applies the clip delta, and queues the changed clips to resolve.
    pub(super) fn apply_clips(&mut self, tables: &Tables) {
        let delta = &tables.clips;
        if delta.is_unchanged() {
            return;
        }
        delta.apply(&mut self.clips);
        if delta.replace {
            self.gpu.clips.clear();
            self.gpu.links.clear();
            self.link_ranges.clear();
            self.link_garbage = 0;
            self.deps.clear();
            self.dirty.clips.mark_all();
            self.dirty.links.mark_all();
            for slot in 0..self.clips.len() as u32 {
                self.enqueue(slot);
            }
            return;
        }
        for &(slot, _) in &delta.changed {
            self.enqueue(slot);
        }
    }

    /// Queues clip `slot` to resolve, unless it is queued already.
    pub(super) fn enqueue(&mut self, slot: u32) {
        let at = slot as usize;
        if self.queued.len() <= at {
            self.queued.resize(at + 1, false);
        }
        if !std::mem::replace(&mut self.queued[at], true) {
            self.queue.push(slot);
        }
    }

    /// Resolves the queued clips, and compacts the link table once abandoned ranges outnumber
    /// live ones.
    pub(super) fn resolve_queued(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        self.gpu.clips.resize(self.clips.len(), GpuClip::default());
        self.link_ranges.resize(self.clips.len(), (0, 0));
        let queue = std::mem::take(&mut self.queue);
        for &slot in &queue {
            self.queued[slot as usize] = false;
            self.resolve_clip(slot);
        }
        self.queue = queue;
        self.queue.clear();
        if self.link_garbage as usize > self.gpu.links.len() / 2 + 1024 {
            self.compact_links();
        }
    }

    /// Resolves clip `slot` into its record and links, and marks what changed.
    fn resolve_clip(&mut self, slot: u32) {
        let at = slot as usize;
        let Some(entry) = self.clips.get(at) else {
            return;
        };
        let resolved = resolve(entry, &self.spaces);
        self.deps.register(slot, &resolved.memberships);

        let mut clip = resolved.clip;
        let count = resolved.links.len() as u32;
        if count > 0 {
            let start = self.link_range(slot, count);
            for (offset, link) in resolved.links.iter().enumerate() {
                let index = start as usize + offset;
                self.gpu.links[index] = *link;
                self.dirty.links.push(index as u32);
            }
            self.gpu.links[start as usize].count = count;
            for MaskLink { link, page, rect } in resolved.masks {
                self.masks.push(MaskRequest {
                    link: start + link as u32,
                    page,
                    rect,
                });
            }
            clip.links = start + 1;
        }
        if self.gpu.clips[at] != clip {
            self.gpu.clips[at] = clip;
            self.dirty.clips.push(slot);
        }
    }

    /// Returns the start of the range for `count` links of clip `slot`.
    ///
    /// A clip keeps its range while its links fit, and otherwise takes a range at the end.
    fn link_range(&mut self, slot: u32, count: u32) -> u32 {
        let (start, capacity) = self.link_ranges[slot as usize];
        if count <= capacity {
            return start;
        }
        self.link_garbage += capacity;
        let start = self.gpu.links.len() as u32;
        self.gpu
            .links
            .resize(start as usize + count as usize, GpuClipLink::default());
        self.link_ranges[slot as usize] = (start, count);
        start
    }

    /// Rebuilds the link table with every clip resolved again.
    fn compact_links(&mut self) {
        self.gpu.links.clear();
        self.link_ranges
            .iter_mut()
            .for_each(|range| *range = (0, 0));
        self.link_garbage = 0;
        self.dirty.links.mark_all();
        self.dirty.clips.mark_all();
        for slot in 0..self.clips.len() as u32 {
            self.resolve_clip(slot);
        }
    }
}
