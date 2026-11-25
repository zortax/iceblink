//! What each image of a kept-target presenter still owes.

use iceblink::STALE_DAMAGE;
use iceblink::damage::DamageSet;

/// The debt of every image of a kept-target presenter.
///
/// An image's debt is the present damage of every frame composed since the image was last
/// written. A new image owes the whole target.
#[derive(Clone, Debug, Default)]
pub struct ImageDebts {
    debts: Vec<DamageSet<STALE_DAMAGE>>,
}

impl ImageDebts {
    /// Returns debts for `images` new images, each owing the whole target.
    pub fn new(images: usize) -> Self {
        Self {
            debts: vec![DamageSet::full(); images],
        }
    }

    /// Adds a composed frame's present damage to every image's debt.
    pub fn composed(&mut self, damage: &DamageSet) {
        for debt in &mut self.debts {
            if damage.is_full() {
                debt.set_full();
                continue;
            }
            for rect in damage.rects() {
                debt.absorb(*rect);
            }
        }
    }

    /// Returns how many images the debts are kept for.
    pub fn len(&self) -> usize {
        self.debts.len()
    }

    /// Reports whether no image's debt is kept.
    pub fn is_empty(&self) -> bool {
        self.debts.is_empty()
    }

    /// Returns what a copy into `image` must cover, or an empty set for an image that does not
    /// exist.
    pub fn debt(&self, image: u32) -> &DamageSet<STALE_DAMAGE> {
        static NONE: DamageSet<STALE_DAMAGE> = DamageSet::new();
        self.debts.get(image as usize).unwrap_or(&NONE)
    }

    /// Clears the debt of `image`, which a present paid.
    pub fn paid(&mut self, image: u32) {
        if let Some(debt) = self.debts.get_mut(image as usize) {
            debt.clear();
        }
    }

    /// Makes `image` owe the whole target, as an image whose contents were lost does.
    pub fn owe_all(&mut self, image: u32) {
        if let Some(debt) = self.debts.get_mut(image as usize) {
            debt.set_full();
        }
    }
}
