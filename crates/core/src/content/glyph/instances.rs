//! Interned variation coordinates, which glyph keys refer to by number.

use std::sync::Arc;

use rustc_hash::FxHashMap;

/// The variation coordinates seen so far, each under a number.
///
/// Number zero is the font's default instance, which an empty coordinate list selects. Numbers
/// are never reused.
#[derive(Debug, Default)]
pub(crate) struct Instances {
    ids: FxHashMap<Arc<[i16]>, u32>,
    coords: Vec<Arc<[i16]>>,
}

impl Instances {
    /// Returns the number of `coords`, interning them if they are new.
    pub(crate) fn intern(&mut self, coords: &[i16]) -> u32 {
        if coords.is_empty() {
            return 0;
        }
        if let Some(&id) = self.ids.get(coords) {
            return id;
        }

        let shared: Arc<[i16]> = Arc::from(coords);
        self.coords.push(Arc::clone(&shared));
        let id = self.coords.len() as u32;
        self.ids.insert(shared, id);
        id
    }

    /// Returns the coordinates of instance `id`, empty for the default instance.
    pub(crate) fn coords(&self, id: u32) -> Arc<[i16]> {
        match id.checked_sub(1) {
            Some(index) => self
                .coords
                .get(index as usize)
                .map_or_else(|| Arc::from([]), Arc::clone),
            None => Arc::from([]),
        }
    }
}
