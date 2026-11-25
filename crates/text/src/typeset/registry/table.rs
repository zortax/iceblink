//! Font ids and the files and blobs they are known by.

use iceblink::{FontBytes, FontData, FontId};
use rustc_hash::FxHashMap;

/// A table of fonts by id, each registered once.
#[derive(Default)]
pub(super) struct Table {
    list: Vec<FontData>,
    ids: FxHashMap<FontData, FontId>,
    /// The files whose faces were added to the collection.
    files: Vec<FontBytes>,
    /// Ids by fontique blob id and face index.
    blobs: FxHashMap<(u64, u32), FontId>,
}

impl Table {
    /// Returns the id of `data`, and whether its file is new to the table.
    ///
    /// A font seen for the first time gets the next id.
    pub(super) fn insert(&mut self, data: FontData) -> (FontId, bool) {
        if let Some(&id) = self.ids.get(&data) {
            return (id, false);
        }

        let id = FontId(self.list.len() as u32);

        let new_file = !self.files.iter().any(|file| same_file(file, &data.file));
        if new_file {
            self.files.push(data.file.clone());
        }
        self.list.push(data.clone());
        self.ids.insert(data, id);
        (id, new_file)
    }

    /// Records that the face `key` of a fontique blob is the font `id`.
    pub(super) fn link_blob(&mut self, key: (u64, u32), id: FontId) {
        self.blobs.insert(key, id);
    }

    pub(super) fn blob(&self, key: (u64, u32)) -> Option<FontId> {
        self.blobs.get(&key).copied()
    }

    pub(super) fn get(&self, id: FontId) -> Option<FontData> {
        self.list.get(id.0 as usize).cloned()
    }

    pub(super) fn len(&self) -> usize {
        self.list.len()
    }
}

fn same_file(a: &FontBytes, b: &FontBytes) -> bool {
    std::sync::Arc::ptr_eq(a, b) || (**a).as_ref() == (**b).as_ref()
}
