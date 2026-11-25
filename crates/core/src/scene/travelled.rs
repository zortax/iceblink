//! The item inks of content a scroll node carries, at a travel of the node.

use iceblink::{DeviceRect, OrderEntry};
use smallvec::SmallVec;

use crate::scene::Scene;
use crate::scene::inks::{ChunkInks, DeviceInks, NO_PIXELS};

/// Appends the device ink of every item of a carried entry with the content moved by `travel`,
/// leaving out the items without one.
///
/// Of a node whose rows are kept, only the rows that meet its still pixels are looked at.
pub(crate) fn item_inks(
    scene: &Scene,
    entry: OrderEntry,
    travel: [i32; 2],
    out: &mut Vec<Option<DeviceRect>>,
) {
    inks_below(scene, entry, travel, out, 0);
}

/// Reports whether a carried entry has an ink with the content moved by `travel`.
pub(crate) fn has_ink(scene: &Scene, entry: OrderEntry, travel: [i32; 2]) -> bool {
    match entry {
        OrderEntry::Chunk(key) => scene
            .chunks
            .get(&key)
            .is_some_and(|chunk| at(scene, &chunk.derived.inks, travel).union().is_some()),
        OrderEntry::Node(id) => scene.orders.at(id.slot()).is_some_and(|node| {
            let still = node
                .ink
                .carry()
                .map(|carry| scene.still_pixels(carry.still));
            node.ink.at(travel, still.unwrap_or(NO_PIXELS)).is_some()
        }),
    }
}

/// Appends the device inks of the items of `entry` at `depth` in the walk, as [`item_inks`] does.
fn inks_below(
    scene: &Scene,
    entry: OrderEntry,
    travel: [i32; 2],
    out: &mut Vec<Option<DeviceRect>>,
    depth: usize,
) {
    match entry {
        OrderEntry::Chunk(key) => {
            if let Some(chunk) = scene.chunks.get(&key) {
                out.extend(
                    at(scene, &chunk.derived.inks, travel)
                        .items()
                        .filter(Option::is_some),
                );
            }
        }
        OrderEntry::Node(id) => {
            let Some(record) = scene.orders.at(id.slot()) else {
                return;
            };
            if depth > scene.orders.span() {
                return;
            }

            let rows = record.ink.carry().zip(record.rows.as_ref());
            let range = match rows {
                Some((carry, rows)) => {
                    let still = scene.still_pixels(carry.still);
                    let top = still.origin.y.saturating_sub(travel[1]);
                    let bottom = top.saturating_add(still.size.height);
                    let meeting = rows.meeting(top, bottom);
                    let entries: SmallVec<[usize; 32]> =
                        meeting.filter_map(|row| rows.entry(row)).collect();
                    entries
                }
                None => (0..record.entries.len()).collect(),
            };

            for index in range {
                if let Some(&child) = record.entries.get(index) {
                    inks_below(scene, child, travel, out, depth + 1);
                }
            }
        }
    }
}

/// Returns a chunk's inks on the device with the content moved by `travel`.
fn at<'a>(scene: &Scene, inks: &'a ChunkInks, travel: [i32; 2]) -> DeviceInks<'a> {
    let still = inks.carry().map(|carry| scene.still_pixels(carry.still));

    inks.at(travel, still.unwrap_or(NO_PIXELS))
}
