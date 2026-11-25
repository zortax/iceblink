//! The font registry the naive compositor starts with.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use iceblink::FontRegistry;
use iceblink::{FontData, FontId};

/// A font registry that keeps every registered font in memory.
///
/// Ids count up from zero in registration order. Registering equal bytes and index again returns
/// the id they were first given.
#[derive(Debug, Default)]
pub struct MemoryFontRegistry {
    fonts: Mutex<Vec<FontData>>,
    generation: AtomicU64,
}

impl MemoryFontRegistry {
    /// Returns a registry with no fonts.
    pub fn new() -> Self {
        Self::default()
    }

    fn fonts(&self) -> MutexGuard<'_, Vec<FontData>> {
        // The list holds no invariant a panic could break.
        self.fonts.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl FontRegistry for MemoryFontRegistry {
    fn register(&self, data: FontData) -> FontId {
        let mut fonts = self.fonts();
        if let Some(index) = fonts.iter().position(|held| *held == data) {
            return FontId(index as u32);
        }

        fonts.push(data);
        self.generation.fetch_add(1, Ordering::Relaxed);
        FontId(fonts.len() as u32 - 1)
    }

    fn get(&self, id: FontId) -> Option<FontData> {
        self.fonts().get(id.0 as usize).cloned()
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
}
