//! The damage of re-recorded chunks: identical replace, differing positions and hints.

use iceblink::{ChunkKey, CompositorEvent, DeviceRect};
use smallvec::SmallVec;

use crate::derive::Derive;
use crate::derive::chunks::reshaped::reshaped;
use crate::derive::diff::{same_frame, same_item};
use crate::derive::route::{Route, damage};
use crate::geometry::bounds::{INK_LIMIT, map};
use crate::geometry::coverage::CoverageChange;
use crate::scene::Scene;
use crate::scene::chunk::ChunkRecord;

/// A re-recorded chunk, with the ink of each old and new item and how its spaces and clips moved.
pub(super) struct Replaced<'a> {
    pub(super) key: ChunkKey,
    /// The record the chunk replaced, or `None` for a new chunk.
    pub(super) original: Option<&'a ChunkRecord>,
    /// The device ink of each item of the record the chunk replaced.
    pub(super) old: &'a [Option<DeviceRect>],
    /// The device ink of each item of the chunk.
    pub(super) inks: &'a [Option<DeviceRect>],
    pub(super) route: Option<Route>,
    pub(super) moved: Moved,
    /// Where the clips the chunk refers to cover differently, when clip writes moved it.
    pub(super) clipped: Option<&'a CoverageChange>,
    /// Whether the chunk refers to an object the commit replaced, which moves it as a write
    /// does and makes no position identical to the record it replaced.
    pub(super) fresh: bool,
}

/// How a re-recorded chunk's spaces and clips moved in the same commit.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Moved {
    /// No move.
    Still,
    /// A move by a scroll offset, whose pending scroll took the chunk's old and new ink.
    Scrolled,
    /// A move by clip writes, with no space they refer to moved.
    Clipped,
    /// A move by a write, after which every item may draw other pixels.
    Redraw,
}

/// Damages the positions of a re-recorded chunk that differ from the chunk it replaced, and
/// reports whether the two record the same items.
///
/// The damage depends on how the chunk moved:
///
/// - If it stayed still, only the differing positions are damaged, cut to the chunk's hint. A quad
///   that changed only its shape is damaged where its coverage changed.
/// - If clip writes moved it, the differing positions are damaged as if it stayed still, without
///   the hint, and its old and new ink are damaged where its clips cover differently.
/// - If a scroll moved it, the differing positions are damaged in full.
/// - If a write or a new offset moved it, or it refers to an object the commit replaced, every
///   position is damaged where it was and where it is.
pub(super) fn rerecorded(derive: &mut Derive<'_>, replaced: Replaced<'_>) -> bool {
    let Replaced {
        key,
        original,
        old: old_inks,
        inks,
        route,
        mut moved,
        clipped,
        fresh,
    } = replaced;
    let Some(new) = derive.scene.chunks.get(&key) else {
        return false;
    };
    let Some(original) = original else {
        for ink in inks.iter().flatten() {
            damage(derive.outputs, route, Some(*ink));
        }
        return false;
    };
    if original.offset != new.offset || fresh {
        moved = Moved::Redraw;
    }

    let frame = !fresh && same_frame(&original.recorded, &new.recorded);
    let len = old_inks.len().max(inks.len());
    if moved == Moved::Clipped && !frame {
        moved = Moved::Redraw;
    }

    let mut differing: SmallVec<[usize; 8]> = SmallVec::new();
    for position in 0..len {
        let same = frame
            && same_item(&original.recorded, &new.recorded, position)
            && (moved != Moved::Still || old_inks.get(position) == inks.get(position));
        if !same {
            differing.push(position);
        }
    }
    let same = frame && differing.is_empty();

    if moved == Moved::Redraw {
        for ink in old_inks.iter().chain(inks) {
            damage(derive.outputs, route, *ink);
        }
        return same;
    }
    if same && moved == Moved::Still {
        derive.counters.chunks_identical += 1;
        return true;
    }

    // The hint holds in the chunk's current space, which a scroll moved its old ink out of.
    let hint = (moved == Moved::Still)
        .then_some(new.recorded.hint)
        .flatten()
        .and_then(|hint| hint_pixels(derive.scene, new, hint));
    let mut full: SmallVec<[DeviceRect; 8]> = SmallVec::new();
    for &position in &differing {
        let old_ink = old_inks.get(position).copied().flatten();
        let ink = inks.get(position).copied().flatten();
        let change = matches!(moved, Moved::Still | Moved::Clipped)
            .then(|| reshaped(derive.scene, original, new, position))
            .flatten();
        match change {
            Some(change) => {
                for ink in old_ink.into_iter().chain(ink) {
                    full.extend(change.cut(ink));
                }
            }
            None => full.extend(old_ink.into_iter().chain(ink)),
        }
    }
    if let Some(clipped) = clipped.filter(|_| moved == Moved::Clipped) {
        // The clips cut the chunk's old and new union, as they cut a chunk they move alone.
        for ink in [union(old_inks), union(inks)].into_iter().flatten() {
            full.extend(clipped.cut(ink));
        }
    }

    let violated = derive.verification
        && hint.is_some_and(|hint| full.iter().any(|ink| !hint.contains_rect(*ink)));
    if violated {
        *derive.hint_violations += 1;
        derive.events.push(CompositorEvent::HintViolated {
            client: derive.scene.client,
            chunk: key,
        });
    }
    for ink in full {
        let cut = match hint {
            Some(hint) if !violated => ink.intersection(hint),
            _ => Some(ink),
        };
        damage(derive.outputs, route, cut);
    }
    same
}

/// Returns the union of `inks`.
fn union(inks: &[Option<DeviceRect>]) -> Option<DeviceRect> {
    inks.iter()
        .flatten()
        .copied()
        .reduce(|union, ink| union.union(ink))
}

/// Returns the device pixels of a chunk's damage hint.
pub(super) fn hint_pixels(
    scene: &Scene,
    record: &ChunkRecord,
    hint: [f32; 4],
) -> Option<DeviceRect> {
    let space = scene.transforms.at(record.recorded.props.space.slot())?;
    map(&space.world, hint).pixels(INK_LIMIT)
}
