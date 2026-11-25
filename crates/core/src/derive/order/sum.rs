//! Summing the inks of an order node's entries into the node's ink.

use iceblink::{DeviceRect, read_extent};
use smallvec::SmallVec;

use crate::derive::order::node::group_region;
use crate::geometry::admits::{UNKNOWN, admits, keeps_whole};
use crate::geometry::bounds::of_pixels;
use crate::geometry::coverage::Partial;
use crate::scene::Scene;
use crate::scene::inks::{Carry, ChunkInks, NodeInk};
use crate::scene::rows::Rows;

/// The inks of an order node's entries, summed into the node's ink.
///
/// A node is carried by a scroll node when that scroll node carries every entry with an ink, all
/// of them cut to the same still pixels, and the node's effect moves with them. Its ink is then
/// the union of the entries' content.
#[derive(Debug)]
pub(crate) struct NodeSum {
    /// The union of the entries' device inks.
    device: Option<DeviceRect>,
    /// The union of the carried entries' content.
    content: Option<DeviceRect>,
    shared: Shared,
    /// The scroll nodes that carry an entry with an ink.
    watching: SmallVec<[u32; 1]>,
    /// The carried entries' content.
    rows: Rows,
    /// Whether the carried entries' content is ordered top to bottom.
    rows_ordered: bool,
    /// The entries' device inks.
    device_rows: Rows,
    /// Whether the entries' device inks are ordered top to bottom.
    device_ordered: bool,
    /// Whether an entry's ink can be wider than the union of its item inks.
    wide: bool,
    /// How many entries were added.
    entries: usize,
}

/// What carries every entry with an ink seen so far.
#[derive(Clone, Copy, Debug)]
enum Shared {
    Nothing,
    One(Carry),
    Several,
}

/// The rows a sum does not keep, emptied by their next user.
pub(crate) type Spare = SmallVec<[Rows; 2]>;

/// An order node's ink as summed from its entries.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Summed {
    pub(crate) ink: NodeInk,
    /// The scroll nodes that carry one of the entries without carrying the node.
    pub(crate) watching: SmallVec<[u32; 1]>,
    /// The vertical extents of the entries with an ink, in content space for a carried node and on
    /// the device for another, when they are ordered.
    pub(crate) rows: Option<Rows>,
}

impl Default for NodeSum {
    fn default() -> Self {
        Self {
            device: None,
            content: None,
            shared: Shared::Nothing,
            watching: SmallVec::new(),
            rows: Rows::default(),
            rows_ordered: true,
            device_rows: Rows::default(),
            device_ordered: true,
            wide: false,
            entries: 0,
        }
    }
}

impl NodeSum {
    /// Returns a sum of no entries that collects its rows into `rows` and `device_rows`, which
    /// must be empty.
    pub(crate) fn with_rows(rows: Rows, device_rows: Rows) -> Self {
        Self {
            rows,
            device_rows,
            ..Self::default()
        }
    }

    /// Adds the next entry, a chunk with inks `inks`.
    pub(crate) fn add_chunk(&mut self, scene: &Scene, inks: &ChunkInks) {
        let carried = inks.carry().zip(inks.content());
        self.add(scene.device_inks(inks).union(), carried);
    }

    /// Adds the next entry, an order node with stored ink `ink` and device ink `device`.
    pub(crate) fn add_node(&mut self, device: Option<DeviceRect>, ink: NodeInk) {
        self.wide |= ink.wide();
        self.add(device, ink.carry().zip(ink.content()));
    }

    /// Adds the next entry, one without a chunk or node record.
    pub(crate) fn skip(&mut self) {
        self.entries += 1;
    }

    fn add(&mut self, device: Option<DeviceRect>, carried: Option<(Carry, DeviceRect)>) {
        let entry = self.entries;
        self.entries += 1;
        self.device = union(self.device, device);
        if let Some(ink) = device
            && self.device_ordered
        {
            self.device_ordered = self.device_rows.push(entry, ink);
        }
        match carried {
            Some((carry, content)) => {
                self.content = union(self.content, Some(content));
                self.shared = match self.shared {
                    Shared::Nothing => Shared::One(carry),
                    Shared::One(held) if held == carry => Shared::One(carry),
                    Shared::One(_) | Shared::Several => Shared::Several,
                };
                if !self.watching.contains(&carry.scroll) {
                    self.watching.push(carry.scroll);
                }
                if self.rows_ordered {
                    self.rows_ordered = self.rows.push(entry, content);
                }
            }
            None if device.is_some() => self.shared = Shared::Several,
            None => {}
        }
    }

    /// Returns the ink of order node `slot` with the entries added.
    ///
    /// The device inks of the node entries of a node that isolates must be exact, since its
    /// written region is computed from them.
    pub(crate) fn finish(self, scene: &Scene, slot: u32) -> Summed {
        self.finish_with_spare(scene, slot).0
    }

    /// Returns the ink of order node `slot` with the entries added, as [`NodeSum::finish`] does,
    /// and the rows the ink does not keep.
    pub(crate) fn finish_with_spare(self, scene: &Scene, slot: u32) -> (Summed, Spare) {
        let isolates = isolates(scene, slot);
        let mut spare = Spare::new();
        let mut kept = |rows: Rows, ordered: bool| {
            if ordered {
                Some(rows)
            } else {
                spare.push(rows);
                None
            }
        };
        match self.shared {
            Shared::One(carry) if moves_with(scene, slot, carry) => {
                let rows = kept(self.rows, self.rows_ordered);
                spare.push(self.device_rows);
                let summed = Summed {
                    ink: NodeInk::carried(self.content, carry),
                    watching: SmallVec::new(),
                    rows,
                };
                (summed, spare)
            }
            _ => {
                let rows = kept(self.device_rows, self.device_ordered);
                spare.push(self.rows);
                let summed = Summed {
                    ink: NodeInk::on_device(
                        group_region(scene, slot, self.device),
                        self.wide && !isolates,
                    ),
                    watching: self.watching,
                    rows,
                };
                (summed, spare)
            }
        }
    }
}

fn union(a: Option<DeviceRect>, b: Option<DeviceRect>) -> Option<DeviceRect> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.union(b)),
        (a, b) => a.or(b),
    }
}

/// Reports whether the effect of the order node at `slot` moves with entries that `carry`
/// carries.
///
/// The effect's filters must read nothing around what they write. The clip of an isolating effect
/// must be still and admit all of the still pixels. The links of a non-isolating effect's clip,
/// which applies to every item, must follow the carrying scroll node or cover all of the still
/// pixels.
fn moves_with(scene: &Scene, slot: u32, carry: Carry) -> bool {
    let Some(effect) = scene
        .orders
        .at(slot)
        .and_then(|record| record.node.effect)
        .and_then(|effect| scene.effects.at(effect.slot()))
    else {
        return true;
    };
    let unit = [0.0, 0.0, 1.0, 1.0];
    let local = read_extent(unit, &effect.node.filters) == unit;
    let still = of_pixels(scene.still_pixels(carry.still));
    let clip = effect.node.clip.map(|clip| clip.slot());
    let admitted = clip.is_none_or(|clip| {
        if effect.node.isolates() {
            admits(scene, clip, still, None)
        } else {
            keeps_whole(scene, clip, still, carry.scroll, false, UNKNOWN) == Some(Partial::NONE)
        }
    });
    local && admitted
}

/// Reports whether the order node at `slot` draws into a group of its own.
pub(crate) fn isolates(scene: &Scene, slot: u32) -> bool {
    scene
        .orders
        .at(slot)
        .and_then(|record| record.node.effect)
        .and_then(|effect| scene.effects.at(effect.slot()))
        .is_some_and(|effect| effect.node.isolates())
}
