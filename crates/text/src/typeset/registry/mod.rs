//! The font registry that layout and the compositor share.

mod table;

use std::sync::{Mutex, MutexGuard, PoisonError};

use fontique::{Blob, Collection, CollectionOptions, SourceCache};
use iceblink::{FontData, FontId};

use self::table::Table;

/// A registry of fonts by id, with the fontique collection that layout looks families up in.
///
/// The collection holds the system fonts if the registry was made with them, and every font
/// registered through [`iceblink::FontRegistry::register`]. A registered font file is shared
/// with the collection and with the compositor, and never copied.
///
/// Fonts get their id when they are registered or when a layout first uses them.
pub struct FontRegistry {
    state: Mutex<State>,
    sources: SourceCache,
}

struct State {
    table: Table,
    collection: Collection,
}

impl FontRegistry {
    /// Returns a registry whose collection holds the fonts installed on the system.
    pub fn new() -> Self {
        Self::with_options(true)
    }

    /// Returns a registry whose collection holds only the fonts registered with it.
    pub fn without_system_fonts() -> Self {
        Self::with_options(false)
    }

    fn with_options(system_fonts: bool) -> Self {
        let collection = Collection::new(CollectionOptions {
            shared: true,
            system_fonts,
        });
        Self {
            state: Mutex::new(State {
                table: Table::default(),
                collection,
            }),
            sources: SourceCache::new_shared(),
        }
    }

    /// Returns a font context over the registry's collection.
    ///
    /// The context also finds fonts registered later.
    pub(crate) fn font_context(&self) -> parley::FontContext {
        parley::FontContext {
            collection: self.state().collection.clone(),
            source_cache: self.sources.clone(),
        }
    }

    /// Returns the id of a font a layout found in the collection, registering it on first use.
    pub(crate) fn id_of(&self, font: &parley::FontData) -> FontId {
        let mut state = self.state();
        let key = (font.data.id(), font.index);
        if let Some(id) = state.table.blob(key) {
            return id;
        }

        let (file, _) = font.data.clone().into_raw_parts();
        let (id, _) = state.table.insert(FontData {
            file,
            index: font.index,
        });
        state.table.link_blob(key, id);
        id
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for FontRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl iceblink::FontRegistry for FontRegistry {
    fn register(&self, data: FontData) -> FontId {
        let mut state = self.state();
        let (id, new_file) = state.table.insert(data.clone());

        if new_file {
            let blob = Blob::new(data.file);
            for (_, faces) in state.collection.register_fonts(blob.clone(), None) {
                for face in faces {
                    if face.index() == data.index {
                        state.table.link_blob((blob.id(), face.index()), id);
                    }
                }
            }
        }
        id
    }

    fn get(&self, id: FontId) -> Option<FontData> {
        self.state().table.get(id)
    }

    fn generation(&self) -> u64 {
        self.state().table.len() as u64
    }
}
