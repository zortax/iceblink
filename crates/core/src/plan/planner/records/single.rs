//! The ordered objects of a kind drawn one at a time.

use iceblink::DeviceRect;

use crate::plan::order::DrawOrder;

/// One object drawn by a batch of its own.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Single {
    pub(crate) order: DrawOrder,
    /// The object's index in its frame array.
    pub(crate) index: u32,
    pub(crate) ink: DeviceRect,
}

/// The drawn objects of one kind that batches draw one at a time.
#[derive(Debug, Default)]
pub(crate) struct Singles {
    items: Vec<Single>,
}

impl Singles {
    pub(crate) fn clear(&mut self) {
        self.items.clear();
    }

    pub(crate) fn push(&mut self, single: Single) {
        self.items.push(single);
    }

    /// Puts the objects into draw order: by order, then index.
    pub(crate) fn sort(&mut self) {
        self.items
            .sort_unstable_by_key(|single| (single.order, single.index));
    }

    pub(crate) fn get(&self, position: usize) -> Option<&Single> {
        self.items.get(position)
    }

    pub(crate) fn iter(&self) -> core::slice::Iter<'_, Single> {
        self.items.iter()
    }
}
