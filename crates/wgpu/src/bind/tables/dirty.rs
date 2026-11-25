//! The slots of a table a frame changed.

/// The slots of one table to upload, or the whole table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DirtySlots {
    /// Whether the whole table is dirty.
    pub(crate) all: bool,
    /// The dirty slots when the whole table is not, sorted and each once after `finish`.
    pub(crate) slots: Vec<u32>,
}

impl DirtySlots {
    pub(crate) fn clear(&mut self) {
        self.all = false;
        self.slots.clear();
    }

    pub(crate) fn mark_all(&mut self) {
        self.all = true;
        self.slots.clear();
    }

    pub(crate) fn push(&mut self, slot: u32) {
        if !self.all {
            self.slots.push(slot);
        }
    }

    /// Sorts the slots and removes duplicates.
    pub(crate) fn finish(&mut self) {
        self.slots.sort_unstable();
        self.slots.dedup();
    }

    /// Returns how many slots are dirty, out of a table of `total`.
    pub(crate) fn count(&self, total: usize) -> u64 {
        if self.all {
            total as u64
        } else {
            self.slots.len() as u64
        }
    }
}

/// The dirty slots of every frame table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DirtyTables {
    pub(crate) clips: DirtySlots,
    pub(crate) links: DirtySlots,
    pub(crate) paints: DirtySlots,
    pub(crate) stops: DirtySlots,
    pub(crate) spatial: DirtySlots,
}

impl DirtyTables {
    pub(crate) fn clear(&mut self) {
        self.clips.clear();
        self.links.clear();
        self.paints.clear();
        self.stops.clear();
        self.spatial.clear();
    }

    pub(crate) fn mark_all(&mut self) {
        self.clips.mark_all();
        self.links.mark_all();
        self.paints.mark_all();
        self.stops.mark_all();
        self.spatial.mark_all();
    }

    pub(crate) fn finish(&mut self) {
        self.clips.finish();
        self.links.finish();
        self.paints.finish();
        self.stops.finish();
        self.spatial.finish();
    }
}
