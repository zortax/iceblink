//! The pending scrolls of content that scroll nodes carry and moved by their travel.

use iceblink::OrderEntry;
use smallvec::SmallVec;

use crate::derive::Derive;
use crate::derive::route::{Route, Routes};
use crate::derive::scroll::{CarriedMove, EntryMove, Moves, Travels, accumulate, defer};
use crate::derive::spread::Spread;
use crate::derive::world::WorldChanges;
use crate::scene::Scene;
use crate::scene::inks::Carry;
use crate::scene::travelled::item_inks;

/// Adds the content every scroll node in `travels` carries to the pending scrolls of the outputs
/// that draw it.
///
/// The moving damage of content whose still pixels are the port's is measured only if a frame
/// refuses the shift. Other content is added with its item inks at the travel before and
/// after the commit.
///
/// The content is taken as the largest carried entries: the entries the scroll node carries of
/// the order nodes it does not carry, and the roots it carries.
pub(crate) fn carried(
    derive: &mut Derive<'_>,
    moves: &mut Moves,
    world: &WorldChanges,
    travels: &Travels,
    routes: &Routes,
) {
    for scroll in travels.scrolls() {
        let before = travels.before(derive.scene, scroll);
        let after = derive.scene.travel(scroll);
        if before == after {
            continue;
        }
        let (mut old, mut new) = (Vec::new(), Vec::new());
        for (route, entry, carry) in entries(derive, scroll, routes) {
            let carried = CarriedMove {
                scroll,
                route,
                entry,
                carry,
                before,
            };
            if defer(derive, moves, world, carried) {
                continue;
            }

            old.clear();
            new.clear();
            item_inks(derive.scene, entry, before, &mut old);
            item_inks(derive.scene, entry, after, &mut new);
            derive.work.moving_inks += (old.len() + new.len()) as u64;
            let moving = EntryMove {
                scroll,
                route,
                entry,
                old: &old,
                new: &new,
            };
            accumulate(derive, moves, world, moving);
        }
    }
}

/// Returns the largest entries scroll node `scroll` carries, with the route and the carry of
/// each.
fn entries(
    derive: &Derive<'_>,
    scroll: u32,
    routes: &Routes,
) -> SmallVec<[(Route, OrderEntry, Carry); 4]> {
    let scene = &*derive.scene;
    let mut entries = SmallVec::new();
    let Some(record) = scene.scrolls.at(scroll) else {
        return entries;
    };
    for &watcher in &record.watchers {
        let Some(route) = routes.of_node(scene, watcher) else {
            continue;
        };
        let Some(node) = scene.orders.at(watcher) else {
            continue;
        };
        for &entry in &node.entries {
            if let Some(carry) = carries(scene, entry, scroll) {
                entries.push((route, entry, carry));
            }
        }
    }
    for (&id, output) in derive.outputs.iter() {
        if let Some((client, root)) = output.root
            && client == scene.client
            && let Some(carry) = carries(scene, OrderEntry::Node(root), scroll)
        {
            let route = Route {
                output: id,
                spread: Spread::default(),
            };
            entries.push((route, OrderEntry::Node(root), carry));
        }
    }
    entries
}

/// Returns the carry of an entry that has an ink, when scroll node `scroll` carries it.
fn carries(scene: &Scene, entry: OrderEntry, scroll: u32) -> Option<Carry> {
    let (carry, content) = match entry {
        OrderEntry::Chunk(key) => {
            let chunk = scene.chunks.get(&key)?;
            (chunk.derived.inks.carry(), chunk.derived.inks.content())
        }
        OrderEntry::Node(id) => {
            let node = scene.orders.at(id.slot())?;
            (node.ink.carry(), node.ink.content())
        }
    };
    carry.filter(|carry| carry.scroll == scroll && content.is_some())
}
