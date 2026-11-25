//! Scans of the order tree for what paints in a shift's region.

use iceblink::{ChunkKey, DeviceRect, OrderEntry};
use rustc_hash::FxHashSet;
use smallvec::SmallVec;

use crate::scene::Scene;
use crate::shift::Refusal;
use crate::shift::rule::cover::covers;
use crate::shift::rule::group::moves_whole;

/// What a backwards scan found.
pub(super) enum Backing {
    /// An opaque solid quad covers the region.
    Found,
    /// Nothing paints in the region.
    Clean,
    /// The scan found a reason to refuse the shift.
    Refused(Refusal),
}

/// Checks an entry inside the content's range: groups that meet the region refuse the shift
/// unless they move as a whole, and chunks that paint in it without moving are collected.
pub(super) fn inside(
    scene: &Scene,
    entry: OrderEntry,
    moving: &FxHashSet<OrderEntry>,
    scroll: u32,
    region: DeviceRect,
    still: &mut SmallVec<[ChunkKey; 4]>,
) -> Result<(), Refusal> {
    if moving.contains(&entry) {
        return Ok(());
    }
    match entry {
        OrderEntry::Chunk(key) => {
            if chunk_paints_in(scene, key, region) {
                still.push(key);
            }
            Ok(())
        }
        OrderEntry::Node(id) => {
            let Some(record) = scene.orders.at(id.slot()) else {
                return Ok(());
            };
            if !meets(scene.node_ink(&record.ink), region) {
                return Ok(());
            }

            if isolates(scene, id.slot()) {
                if moves_whole(scene, id.slot(), moving, scroll, region) {
                    return Ok(());
                }
                return Err(Refusal::Overdrawn);
            }

            for entry in &record.entries {
                inside(scene, *entry, moving, scroll, region, still)?;
            }
            Ok(())
        }
    }
}

/// Reports whether anything in an entry's subtree paints in `region`.
pub(super) fn paints_in(scene: &Scene, entry: OrderEntry, region: DeviceRect) -> bool {
    match entry {
        OrderEntry::Chunk(key) => chunk_paints_in(scene, key, region),
        OrderEntry::Node(id) => scene.orders.at(id.slot()).is_some_and(|record| {
            // The subtree ink dismisses a whole subtree that misses the region, and a group that
            // meets it changes the composite there.
            meets(scene.node_ink(&record.ink), region)
                && (isolates(scene, id.slot())
                    || record
                        .entries
                        .iter()
                        .any(|entry| paints_in(scene, *entry, region)))
        }),
    }
}

/// Reports whether an item of chunk `key` paints in `region`.
fn chunk_paints_in(scene: &Scene, key: ChunkKey, region: DeviceRect) -> bool {
    scene.chunks.get(&key).is_some_and(|record| {
        let inks = scene.device_inks(&record.derived.inks);
        meets(inks.union(), region) && inks.items().any(|ink| meets(ink, region))
    })
}

/// Scans an entry backwards for what backs the region.
pub(super) fn backing(scene: &Scene, entry: OrderEntry, region: DeviceRect) -> Backing {
    match entry {
        OrderEntry::Chunk(key) => chunk_backing(scene, key, region),
        OrderEntry::Node(id) => {
            let Some(record) = scene.orders.at(id.slot()) else {
                return Backing::Clean;
            };
            if !meets(scene.node_ink(&record.ink), region) {
                return Backing::Clean;
            }
            if isolates(scene, id.slot()) {
                return Backing::Refused(Refusal::Overdrawn);
            }

            for entry in record.entries.iter().rev() {
                match backing(scene, *entry, region) {
                    Backing::Clean => {}
                    found => return found,
                }
            }
            Backing::Clean
        }
    }
}

/// Scans the items of chunk `key` backwards for what backs `region`.
fn chunk_backing(scene: &Scene, key: ChunkKey, region: DeviceRect) -> Backing {
    let Some(record) = scene.chunks.get(&key) else {
        return Backing::Clean;
    };
    let inks = scene.device_inks(&record.derived.inks);
    if !meets(inks.union(), region) {
        return Backing::Clean;
    }

    for position in (0..inks.len()).rev() {
        if !meets(inks.item(position), region) {
            continue;
        }
        return if covers(scene, key, position, region) {
            Backing::Found
        } else {
            Backing::Refused(Refusal::NotBacked)
        };
    }
    Backing::Clean
}

/// Reports whether `ink` exists and meets `region`.
fn meets(ink: Option<DeviceRect>, region: DeviceRect) -> bool {
    ink.is_some_and(|ink| ink.intersects(region))
}

/// Reports whether the order node at `slot` draws into a group of its own.
pub(super) fn isolates(scene: &Scene, slot: u32) -> bool {
    scene
        .orders
        .at(slot)
        .and_then(|record| record.node.effect)
        .and_then(|effect| scene.effects.at(effect.slot()))
        .is_some_and(|effect| effect.node.isolates())
}
