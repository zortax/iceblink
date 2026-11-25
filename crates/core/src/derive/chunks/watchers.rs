//! The watchers of a scroll node whose sums a travel changes.

use iceblink::OrderEntry;

use crate::derive::scroll::Travels;
use crate::scene::Scene;

/// Reports whether the travel of scroll node `scroll` changed the device ink of an entry of the
/// order node `watcher`.
///
/// Only entries that `scroll` carries move with its travel. Carried content that covers its still
/// pixels before and after the travel keeps its device ink.
pub(super) fn moved_by(scene: &Scene, travels: &Travels, watcher: u32, scroll: u32) -> bool {
    let Some(record) = scene.orders.at(watcher) else {
        return false;
    };
    let before = travels.before(scene, scroll);
    let now = scene.travel(scroll);
    record.entries.iter().any(|entry| match *entry {
        OrderEntry::Chunk(key) => scene.chunks.get(&key).is_some_and(|chunk| {
            let inks = &chunk.derived.inks;
            inks.carry().is_some_and(|carry| {
                let still = scene.still_pixels(carry.still);
                carry.scroll == scroll
                    && inks.at(before, still).union() != inks.at(now, still).union()
            })
        }),
        OrderEntry::Node(id) => scene.orders.at(id.slot()).is_some_and(|child| {
            let ink = &child.ink;
            ink.carry().is_some_and(|carry| {
                let still = scene.still_pixels(carry.still);
                carry.scroll == scroll && ink.at(before, still) != ink.at(now, still)
            })
        }),
    })
}
