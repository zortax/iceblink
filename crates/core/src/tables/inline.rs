//! Frame paint slots of the inline colours recorded chunks use.

use iceblink::{Color, FramePaint};
use rustc_hash::FxHashMap;

use crate::tables::table::FrameTable;

/// The frame paint slot of every inline colour a recorded chunk uses.
///
/// A colour keeps one slot while any recording uses it, so equal colours share a slot. The slot is
/// released once the last recording that used the colour is dropped.
#[derive(Debug, Default)]
pub(crate) struct InlinePaints {
    held: FxHashMap<[u8; 4], Held>,
}

/// The frame paint slot of one inline colour, and how many recordings use it.
#[derive(Debug)]
struct Held {
    slot: u32,
    /// The recordings that use the colour.
    users: u32,
}

impl InlinePaints {
    /// Returns the frame paint slot of `rgba`, if a recording uses the colour.
    pub(crate) fn slot(&self, rgba: [u8; 4]) -> Option<u32> {
        self.held.get(&rgba).map(|held| held.slot)
    }

    /// Adds a recording that uses `rgba`, and stores the colour in `paints` for the first.
    pub(crate) fn hold(&mut self, paints: &mut FrameTable<FramePaint>, rgba: [u8; 4]) {
        self.held
            .entry(rgba)
            .and_modify(|held| held.users += 1)
            .or_insert_with(|| Held {
                slot: paints.allocate(FramePaint::Solid(Color::from_rgba8(rgba))),
                users: 1,
            });
    }

    /// Removes a recording that used `rgba`, and releases the colour's slot at build serial
    /// `serial` when no recording uses it any more.
    pub(crate) fn release(
        &mut self,
        paints: &mut FrameTable<FramePaint>,
        rgba: [u8; 4],
        serial: u64,
    ) {
        let Some(held) = self.held.get_mut(&rgba) else {
            return;
        };

        held.users -= 1;
        if held.users == 0 {
            paints.release(held.slot, serial);
            self.held.remove(&rgba);
        }
    }
}
