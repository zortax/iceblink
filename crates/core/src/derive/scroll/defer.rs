//! Carried content whose moving damage waits for a frame that refuses the shift.

use iceblink::OrderEntry;

use crate::derive::Derive;
use crate::derive::route::Route;
use crate::derive::scroll::admit::carry_keeps;
use crate::derive::scroll::{Moves, moves_with, pending_scroll, whole_pixels};
use crate::derive::world::WorldChanges;
use crate::scene::inks::Carry;
use crate::scene::travelled::has_ink;
use crate::shift::port_pixels;

/// Adds an entry that scroll node `scroll` carries and moved by its travel to the output's pending
/// scroll, with its inks left to be measured when a frame refuses the shift.
///
/// Returns `false`, adding nothing, when the entry does not move rigidly with the node, or its
/// still links cut it to other pixels than the port's.
pub(crate) fn defer(
    derive: &mut Derive<'_>,
    moves: &mut Moves,
    world: &WorldChanges,
    carried: CarriedMove,
) -> bool {
    let CarriedMove {
        scroll,
        route,
        entry,
        carry,
        before,
    } = carried;

    let by = moves.of(derive, world, scroll);
    let rigid = by.and_then(whole_pixels).is_some()
        && match entry {
            OrderEntry::Chunk(key) => moves_with(derive, world, key, by),
            OrderEntry::Node(_) => true,
        };
    let kept = port_pixels(derive.scene, scroll).and_then(|port| {
        port.contains_rect(derive.scene.still_pixels(carry.still))
            .then(|| carry_keeps(derive.scene, carry, port))?
    });
    let Some(partial) = kept.filter(|_| rigid) else {
        return false;
    };

    let after = derive.scene.travel(scroll);
    let inked = has_ink(derive.scene, entry, before) || has_ink(derive.scene, entry, after);
    let Some(pending) = pending_scroll(derive, moves, scroll, route.output, by) else {
        return true;
    };
    pending.partial = pending.partial.and(partial);
    if inked {
        pending.entries.insert(entry);
        pending.spreads |= route.spread.reaches();
        pending.moving.defer(scroll, before, entry, route.spread);
    }
    true
}

/// An entry a scroll node carries, moved by the node's travel from `before`.
pub(crate) struct CarriedMove {
    pub(crate) scroll: u32,
    pub(crate) route: Route,
    pub(crate) entry: OrderEntry,
    /// How the scroll node carries the entry.
    pub(crate) carry: Carry,
    pub(crate) before: [i32; 2],
}
