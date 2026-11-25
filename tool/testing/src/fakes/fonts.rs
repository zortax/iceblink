//! A font registry held in memory.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use iceblink::FontRegistry;
use iceblink::{FontData, FontId};

/// A font registry that keeps every registered font in memory and checks no bytes.
///
/// Ids count up from zero in registration order, and registering the same data again returns its
/// first id.
#[derive(Debug, Default)]
pub struct FakeFontRegistry {
    fonts: Mutex<Vec<FontData>>,
    generation: AtomicU64,
}

impl FakeFontRegistry {
    /// Returns a registry with no fonts.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns how many fonts are registered.
    pub fn len(&self) -> usize {
        self.held().len()
    }

    /// Reports whether no font is registered.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn held(&self) -> std::sync::MutexGuard<'_, Vec<FontData>> {
        self.fonts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl FontRegistry for FakeFontRegistry {
    fn register(&self, data: FontData) -> FontId {
        let mut fonts = self.held();
        if let Some(index) = fonts.iter().position(|held| *held == data) {
            return FontId(index as u32);
        }

        fonts.push(data);
        self.generation.fetch_add(1, Ordering::Relaxed);
        FontId(fonts.len() as u32 - 1)
    }

    fn get(&self, id: FontId) -> Option<FontData> {
        self.held().get(id.0 as usize).cloned()
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
}

/// Returns font data whose bytes are `seed`, so different seeds are different fonts.
pub fn fake_font(seed: u32) -> FontData {
    FontData::new(seed.to_le_bytes(), 0)
}
