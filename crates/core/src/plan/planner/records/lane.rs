//! The ordered instances of one lane.

use iceblink::{DeviceRect, ItemSource};

use crate::plan::order::DrawOrder;

/// One drawn instance of a lane.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Placed {
    pub(crate) source: ItemSource,
    pub(crate) ink: DeviceRect,
    /// The index of the instance's opaque cover, or `NO_COVER`.
    pub(crate) cover: u32,
}

/// The drawn instances of one lane, in push order, and their draw sequence once sorted.
#[derive(Debug, Default)]
pub(crate) struct LaneRecords {
    placed: Vec<Placed>,
    /// One sort key per instance, which packs the order, the two binding words and the push index
    /// from the high bits down, so sorting the keys sorts the lane.
    keys: Vec<u128>,
}

impl LaneRecords {
    pub(crate) fn clear(&mut self) {
        self.placed.clear();
        self.keys.clear();
    }

    /// Adds an instance at `order` with the binding `key`, and returns its push index.
    pub(crate) fn push(&mut self, order: DrawOrder, key: [u32; 2], placed: Placed) -> u32 {
        let index = self.placed.len() as u32;
        self.placed.push(placed);
        self.keys.push(
            (u128::from(order) << 96)
                | (u128::from(key[0]) << 64)
                | (u128::from(key[1]) << 32)
                | u128::from(index),
        );
        index
    }

    /// Puts the instances into draw order: by order, then key, then push order.
    pub(crate) fn sort(&mut self) {
        self.keys.sort_unstable();
    }

    /// Returns the order of the instance at draw position `position`.
    pub(crate) fn order_at(&self, position: usize) -> Option<DrawOrder> {
        self.keys.get(position).map(|key| (key >> 96) as DrawOrder)
    }

    /// Returns the binding key of the instance at draw position `position`.
    pub(crate) fn key_at(&self, position: usize) -> [u32; 2] {
        let key = self.keys[position];
        [(key >> 64) as u32, (key >> 32) as u32]
    }

    /// Returns the push index of the instance at draw position `position`.
    pub(crate) fn index_at(&self, position: usize) -> u32 {
        self.keys[position] as u32
    }

    /// Returns the instance at draw position `position`.
    pub(crate) fn placed_at(&self, position: usize) -> &Placed {
        &self.placed[self.index_at(position) as usize]
    }

    /// Writes the sources in draw order into `sources`, joining runs that continue each other.
    pub(crate) fn write_sources(&self, sources: &mut Vec<ItemSource>) {
        sources.clear();
        for key in &self.keys {
            let source = self.placed[*key as u32 as usize].source;
            match sources.last_mut() {
                Some(last)
                    if last.chunk == source.chunk
                        && last.index.checked_add(last.len) == Some(source.index) =>
                {
                    last.len += source.len;
                }
                _ => sources.push(source),
            }
        }
    }

    /// Reports whether a record draws more than one instance, so that draw positions and
    /// instance positions differ.
    pub(crate) fn has_runs(&self) -> bool {
        self.placed.iter().any(|placed| placed.source.len > 1)
    }

    /// Writes into `starts` the instance position of each draw position, and the number of
    /// instances after the last.
    pub(crate) fn starts(&self, starts: &mut Vec<u32>) {
        starts.clear();
        let mut at = 0u32;
        for key in &self.keys {
            starts.push(at);
            at += self.placed[*key as u32 as usize].source.len;
        }
        starts.push(at);
    }

    /// Writes each push index's draw position into `positions`.
    pub(crate) fn positions(&self, positions: &mut Vec<u32>) {
        positions.clear();
        positions.resize(self.keys.len(), 0);
        for (position, key) in self.keys.iter().enumerate() {
            positions[*key as u32 as usize] = position as u32;
        }
    }

    /// Returns the orders of the instances in draw order.
    pub(crate) fn orders(&self) -> impl Iterator<Item = DrawOrder> + '_ {
        self.keys.iter().map(|key| (key >> 96) as DrawOrder)
    }

    /// Calls `each` with the order and ink of every instance.
    pub(crate) fn for_each(&self, mut each: impl FnMut(DrawOrder, DeviceRect)) {
        for key in &self.keys {
            each(
                (key >> 96) as DrawOrder,
                self.placed[*key as u32 as usize].ink,
            );
        }
    }
}
