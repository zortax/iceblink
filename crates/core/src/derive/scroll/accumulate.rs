//! The pending scrolls of entries that only a scroll offset change moved.

use iceblink::{DeviceRect, OrderEntry};

use crate::derive::Derive;
use crate::derive::route::Route;
use crate::derive::scroll::admit::keeps_port;
use crate::derive::scroll::{
    Moves, moves_with, pending_scroll, same_translation, translation_between, whole_pixels,
};
use crate::derive::world::WorldChanges;
use crate::scene::inks::moved;
use crate::shift::port_pixels;

/// A chunk, or an order node the scroll node carries, that only a scroll offset change moved,
/// with its inks before and after.
pub(crate) struct EntryMove<'a> {
    pub(crate) scroll: u32,
    pub(crate) route: Route,
    pub(crate) entry: OrderEntry,
    pub(crate) old: &'a [Option<DeviceRect>],
    pub(crate) new: &'a [Option<DeviceRect>],
}

/// Adds the old and new inks of an entry that only scroll node `scroll` moved to the output's
/// pending scroll, sorted into the content that moved with it and the content that did not.
///
/// The inks are grown by the spread of the groups around the entry.
pub(crate) fn accumulate(
    derive: &mut Derive<'_>,
    moves: &mut Moves,
    world: &WorldChanges,
    moving: EntryMove<'_>,
) {
    let EntryMove {
        scroll,
        route,
        entry,
        old,
        new,
    } = moving;

    let by = moves.of(derive, world, scroll);
    let port = port_pixels(derive.scene, scroll);
    let whole = by.and_then(whole_pixels);
    // An entry moves rigidly when every space it is drawn in moved by the same translation. Every
    // order node entry is one the scroll node carries.
    let rigid = whole.is_some()
        && match entry {
            OrderEntry::Chunk(key) => moves_with(derive, world, key, by),
            OrderEntry::Node(_) => true,
        };
    let kept = port.and_then(|port| {
        let inside = old
            .iter()
            .chain(new)
            .flatten()
            .all(|ink| port.contains_rect(*ink));
        // A space that follows several offsets moved with the scroll node when the commit moved it
        // by the node's move, and stayed when the commit left it.
        let moved = |space: u32| match world.moved.get(&space) {
            Some(change) => translation_between(&change.before, &change.after)
                .zip(by)
                .is_some_and(|(delta, by)| same_translation(delta, by))
                .then_some(true),
            None => Some(false),
        };
        inside.then(|| keeps_port(derive.scene, entry, scroll, port, &moved))?
    });

    let Some(pending) = pending_scroll(derive, moves, scroll, route.output, by) else {
        return;
    };
    let spread = route.spread;
    if rigid {
        match kept {
            Some(partial) => pending.partial = pending.partial.and(partial),
            None => pending.contained = false,
        }
        if old.iter().chain(new).any(Option::is_some) {
            pending.entries.insert(entry);
            pending.spreads |= spread.reaches();
        }
        for ink in old.iter().chain(new).flatten() {
            pending.moving.absorb(spread.grow(*ink));
        }
    } else {
        // Content that does not move with the node is damaged where it was, where it is, and
        // where the shift would carry it.
        for ink in old.iter().chain(new).flatten() {
            pending.still.absorb(spread.grow(*ink));
        }
        if let Some(shift) = whole {
            for ink in old.iter().flatten() {
                pending.still.absorb(spread.grow(moved(*ink, shift)));
            }
        }
    }
}
