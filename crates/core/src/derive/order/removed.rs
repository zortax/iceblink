//! The damage of what a commit's splices removed, measured before the commit.

use core::cell::OnceCell;

use iceblink::{ChunkKey, DeviceRect, OrderEntry};

use crate::commit::journal::{ChunkChange, Journal, Op, OrderChange};
use crate::derive::Derive;
use crate::derive::order::{INLINE, exact_ink, exact_ink_of, redrawn_nodes};
use crate::derive::route::{Routes, damage};
use crate::derive::spread::Spread;
use crate::scene::Scene;
use crate::scene::record::OrderRecord;
use crate::small::SmallMap;

/// Damages what splices removed and what effect writes redraw, measured before the commit.
pub(crate) fn removed_damage(derive: &mut Derive<'_>, journal: &Journal, routes: &Routes) {
    let mut originals = Originals::default();
    original_inks(derive.scene, journal, &mut originals);
    let removed = RemovedNodes::of(journal);
    for change in &journal.orders {
        let OrderChange::Splice(splice) = change else {
            continue;
        };
        if !splice.inked {
            continue;
        }
        for entry in journal.removed(splice) {
            let ink =
                match entry {
                    OrderEntry::Chunk(key) => match originals.get(key) {
                        Some(ink) => *ink,
                        None => derive.scene.chunks.get(key).and_then(|record| {
                            derive.scene.device_inks(&record.derived.inks).union()
                        }),
                    },
                    OrderEntry::Node(id) => node_ink(derive.scene, &removed, id.slot()),
                };
            if let Some(ink) = ink {
                let spread = Spread::inside(derive.scene, splice.node);
                routes.damage_all(derive.outputs, spread.grow(ink));
            }
        }
    }

    for slot in redrawn_nodes(derive.scene, journal) {
        let ink = node_ink(derive.scene, &removed, slot);
        let route = routes.of_group(derive.scene, slot);
        damage(derive.outputs, route, ink);
    }
}

/// The ink each replaced chunk had before the commit, by key.
type Originals = SmallMap<ChunkKey, Option<DeviceRect>, INLINE>;

/// Adds the ink each replaced chunk had before the commit to `inks`.
fn original_inks(scene: &Scene, journal: &Journal, inks: &mut Originals) {
    for change in &journal.chunks {
        if let ChunkChange::Replaced { key, before, .. } = change {
            inks.get_or_insert_with(*key, || {
                journal
                    .before(*before)
                    .and_then(|record| scene.device_inks(&record.derived.inks).union())
            });
        }
    }
}

/// The order nodes a commit removed, indexed by slot on the first lookup.
struct RemovedNodes<'a> {
    journal: &'a Journal,
    /// The position of each removal in `Journal::order_nodes`, by slot, or `u32::MAX`.
    index: OnceCell<Vec<u32>>,
}

impl<'a> RemovedNodes<'a> {
    fn of(journal: &'a Journal) -> Self {
        Self {
            journal,
            index: OnceCell::new(),
        }
    }

    /// Returns the record the commit removed from `slot`.
    fn get(&self, slot: u32) -> Option<&'a OrderRecord> {
        let index = self.index.get_or_init(|| {
            let removals = self.journal.order_nodes.iter().enumerate();
            let removals = removals.filter(|(_, change)| matches!(change.op, Op::Removed(_)));
            let span = removals.clone().map(|(_, change)| change.slot + 1).max();
            let mut index = vec![u32::MAX; span.unwrap_or(0) as usize];
            for (at, change) in removals {
                index[change.slot as usize] = at as u32;
            }
            index
        });
        let at = *index.get(slot as usize)?;
        match self.journal.order_nodes.get(at as usize)?.op {
            Op::Removed(index) => self.journal.removed_order(index),
            Op::Created(_) | Op::Wrote(_) => None,
        }
    }
}

/// Returns the ink of an order node, from the scene or from the record a removal took out.
fn node_ink(scene: &Scene, removed: &RemovedNodes<'_>, slot: u32) -> Option<DeviceRect> {
    match scene.orders.at(slot) {
        Some(_) => exact_ink(scene, slot),
        None => exact_ink_of(scene, removed.get(slot)?),
    }
}
