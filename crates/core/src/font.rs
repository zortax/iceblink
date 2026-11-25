//! The font registry a compositor keeps when the client shares none.

use std::sync::{Mutex, PoisonError};

use iceblink::FontRegistry;
use iceblink::{FontData, FontId};
use rustc_hash::FxHashMap;

/// Fonts by id, registered once each and never removed.
#[derive(Debug, Default)]
pub(crate) struct OwnFonts {
    fonts: Mutex<Fonts>,
}

#[derive(Debug, Default)]
struct Fonts {
    list: Vec<FontData>,
    ids: FxHashMap<FontData, FontId>,
}

impl FontRegistry for OwnFonts {
    fn register(&self, data: FontData) -> FontId {
        let mut fonts = self.fonts.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(&id) = fonts.ids.get(&data) {
            return id;
        }

        let id = FontId(fonts.list.len() as u32);
        fonts.list.push(data.clone());
        fonts.ids.insert(data, id);
        id
    }

    fn get(&self, id: FontId) -> Option<FontData> {
        let fonts = self.fonts.lock().unwrap_or_else(PoisonError::into_inner);
        fonts.list.get(id.0 as usize).cloned()
    }

    fn generation(&self) -> u64 {
        let fonts = self.fonts.lock().unwrap_or_else(PoisonError::into_inner);
        fonts.list.len() as u64
    }
}
