//! Whether a scroll port's pixels may be moved instead of drawn again.
//!
//! The composed pixels inside a port are the composite of everything drawn there, so moving them
//! moves all of it. That is the same picture only when what the content is drawn over is one
//! opaque colour, and nothing outside the content draws in the port.

mod cover;
mod group;
mod path;
mod scan;

use iceblink::{ChunkKey, DeviceRect, OrderEntry};
use rustc_hash::FxHashSet;
use smallvec::SmallVec;

use self::path::{ancestors, content_range, shared};
use self::scan::{Backing, backing, inside, isolates, paints_in};
use crate::geometry::admits::{UNKNOWN, keeps_whole};
use crate::geometry::bounds::of_pixels;
use crate::geometry::coverage::Partial;
use crate::scene::Scene;
use crate::shift::Refusal;

/// Checks the order-tree conditions of the shift rule for content `moving` of the output whose
/// root is the order node at `root`, and returns the chunks inside the content that do not move.
///
/// The moving content is a set of chunks and of order nodes whose whole subtree moves, and it
/// lies under the deepest order node above all of them. Inside the content, an isolating group
/// that meets `region` must move with scroll node `scroll` as a whole. Every chunk drawn after
/// the content must miss `region`, and so must every isolating group around or beside the content
/// and every reading composite. The clip of every effect around the content must hold all of
/// `region`, and where it covers the region in part is added to `partial`. Scanning backwards from
/// the content, the first thing drawn in the region must be an opaque solid quad that covers it.
pub(crate) fn check(
    scene: &Scene,
    root: u32,
    moving: &FxHashSet<OrderEntry>,
    scroll: u32,
    region: DeviceRect,
    partial: &mut Partial,
) -> Result<SmallVec<[ChunkKey; 4]>, Refusal> {
    let mut entries = moving.iter();
    let first = *entries.next().ok_or(Refusal::NotDrawn)?;

    let reads = |entry: &OrderEntry| match entry {
        OrderEntry::Chunk(key) => scene.reading.contains(key),
        OrderEntry::Node(_) => false,
    };
    if moving.iter().any(reads) {
        return Err(Refusal::Read);
    }

    let root_id = scene.orders.at(root).ok_or(Refusal::NotDrawn)?.id;
    if moving.contains(&OrderEntry::Node(root_id)) {
        // Nothing is drawn beneath content that is the whole output.
        return Err(Refusal::NotBacked);
    }

    let path = ancestors(scene, first, root).ok_or(Refusal::NotDrawn)?;
    let mut paths: SmallVec<[SmallVec<[u32; 8]>; 2]> = SmallVec::new();
    paths.push(path.clone());
    let mut common = 0;
    for &entry in entries {
        let other = ancestors(scene, entry, root).ok_or(Refusal::NotDrawn)?;
        common = common.max(shared(&path, &other));
        paths.push(other);
    }
    // The content is the deepest node above every moving entry. `path[0]` is the first entry's
    // own parent.
    let content = path[common];

    let (low, high) = content_range(scene, content, moving, &paths).ok_or(Refusal::NotDrawn)?;

    let mut still = SmallVec::new();
    let entries = &scene.orders.at(content).ok_or(Refusal::NotDrawn)?.entries;
    for entry in &entries[low..=high] {
        inside(scene, *entry, moving, scroll, region, &mut still)?;
    }

    above(scene, &path[common..], low, high, scroll, region, partial)?;
    Ok(still)
}

/// Checks the order nodes `levels`, from the content node up to the root, around the content's
/// moving entries at positions `low..=high`.
///
/// No node may isolate, and the clip of each node's effect must hold all of `region`. Where the
/// clip covers the region in part is added to `partial`. Everything drawn after the content must
/// miss `region`, and the first thing drawn in it before must back it.
fn above(
    scene: &Scene,
    levels: &[u32],
    low: usize,
    high: usize,
    scroll: u32,
    region: DeviceRect,
    partial: &mut Partial,
) -> Result<(), Refusal> {
    let mut child = None;
    let mut backed = false;
    for &level in levels {
        if isolates(scene, level) {
            return Err(Refusal::Overdrawn);
        }

        let record = scene.orders.at(level).ok_or(Refusal::NotDrawn)?;
        let clip = record
            .node
            .effect
            .and_then(|effect| scene.effects.at(effect.slot()))
            .and_then(|effect| effect.node.clip);
        if let Some(clip) = clip {
            let kept = keeps_whole(scene, clip.slot(), of_pixels(region), scroll, true, UNKNOWN);
            *partial = partial.and(kept.ok_or(Refusal::Uncontained)?);
        }

        let entries = &record.entries;
        let (before, after) = match child {
            None => (low, high + 1),
            Some(child) => {
                let at = entries
                    .iter()
                    .position(|entry| *entry == OrderEntry::Node(child))
                    .ok_or(Refusal::NotDrawn)?;
                (at, at + 1)
            }
        };

        for entry in &entries[after..] {
            if paints_in(scene, *entry, region) {
                return Err(Refusal::Overdrawn);
            }
        }
        if !backed {
            for entry in entries[..before].iter().rev() {
                match backing(scene, *entry, region) {
                    Backing::Found => {
                        backed = true;
                        break;
                    }
                    Backing::Clean => {}
                    Backing::Refused(refusal) => return Err(refusal),
                }
            }
        }

        child = scene.orders.at(level).map(|record| record.id);
    }

    if backed {
        Ok(())
    } else {
        Err(Refusal::NotBacked)
    }
}
