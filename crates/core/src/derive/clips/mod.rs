//! Bounds, frame entries and coverage changes of the clip nodes a commit made stale.

mod coverage;

use iceblink::{ClipNode, DeviceRect};
use rustc_hash::{FxHashMap, FxHashSet};

use self::coverage::own_change;
use crate::derive::Derive;
use crate::derive::carry::whole_pixels;
use crate::derive::links::Cause;
use crate::derive::world::WorldChanges;
use crate::geometry::bounds::{DeviceBox, INK_LIMIT};
use crate::geometry::clip::node_bounds;
use crate::geometry::coverage::CoverageChange;
use crate::geometry::split::node_split;
use crate::scene::Scene;
use crate::scene::follows::{Follows, StillRoot};
use crate::scene::inks::{ChunkInks, DeviceInks, NO_PIXELS, Still};
use crate::tables::clip::node_chain;

/// The clip nodes whose bounds or links changed, with why and where their chain's coverage
/// changed.
#[derive(Debug, Default)]
pub(crate) struct ClipChanges {
    /// The cause of each changed node's change, by slot.
    pub(crate) changed: FxHashMap<u32, Cause>,
    /// The device pixels where each changed node's chain covers differently, by slot.
    pub(crate) coverage: FxHashMap<u32, CoverageChange>,
    /// Each changed node's chain before the commit, by slot.
    pub(crate) before: FxHashMap<u32, ChainBefore>,
}

impl ClipChanges {
    /// Returns the device pixels carried content cut to `still` was cut to before the commit.
    ///
    /// They are every pixel for a clip node the commit removed, whose chain the changes do not
    /// hold.
    pub(crate) fn still_before(&self, scene: &Scene, still: Still) -> DeviceRect {
        match still {
            Still::Node(slot) => match self.before.get(&slot) {
                Some(before) => before.still.unwrap_or(NO_PIXELS),
                None if scene.clips.at(slot).is_none() => INK_LIMIT,
                None => scene.still_pixels(still),
            },
            Still::Fixed { pixels, .. } => pixels,
        }
    }

    /// Returns a chunk's inks on the device before the commit, with the content of a carried
    /// chunk moved by `travel`.
    pub(crate) fn device_inks_before<'a>(
        &self,
        scene: &Scene,
        inks: &'a ChunkInks,
        travel: [i32; 2],
    ) -> DeviceInks<'a> {
        match inks.carry() {
            Some(carry) => inks.at(travel, self.still_before(scene, carry.still)),
            None => inks.at(travel, NO_PIXELS),
        }
    }
}

/// A clip chain's bounds and what its spaces follow, from before a commit.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ChainBefore {
    pub(crate) bounds: DeviceBox,
    pub(crate) follows: Follows,
    /// The link that decided the chain's still bounds.
    pub(crate) root: StillRoot,
    /// The whole pixels of the chain's still bounds, as `ClipRecord::still` holds them.
    pub(crate) still: Option<DeviceRect>,
}

/// Recomputes the bounds, splits and coverage changes of the clips `written` by the commit, of
/// the clips whose space moved, and of their descendants, and rewrites the frame entries of the
/// written clips' chains.
///
/// `written` holds each written clip with its value before the commit, `None` for a clip the
/// commit created.
pub(crate) fn refresh(
    derive: &mut Derive<'_>,
    written: &[(u32, Option<ClipNode>)],
    world: &WorldChanges,
) -> ClipChanges {
    let scene = &mut *derive.scene;
    let mut starts: Vec<(u32, Cause, bool)> = written
        .iter()
        .map(|&(slot, _)| (slot, Cause::Write, true))
        .collect();
    for moved in world.moved.values() {
        if let Some(record) = scene.transforms.at(moved.slot) {
            starts.extend(record.clips.iter().map(|&clip| (clip, moved.cause, false)));
        }
    }

    let before: FxHashMap<u32, Option<&ClipNode>> = written
        .iter()
        .map(|(slot, before)| (*slot, before.as_ref()))
        .collect();
    let written: FxHashSet<u32> = before.keys().copied().collect();
    let mut starts: Vec<(usize, u32, Cause, bool)> = starts
        .into_iter()
        .map(|(slot, cause, links)| (depth(scene, slot), slot, cause, links))
        .collect();
    starts.sort_unstable_by_key(|&(depth, slot, ..)| (depth, slot));

    let mut changes = ClipChanges::default();
    let mut stack = Vec::new();
    for (_, start, cause, links) in starts {
        if changes.changed.contains_key(&start) {
            continue;
        }
        stack.push((start, cause, links));
        while let Some((slot, inherited, links)) = stack.pop() {
            if changes.changed.contains_key(&slot) {
                continue;
            }
            let links = links || written.contains(&slot);
            let cause = if links { Cause::Write } else { inherited };
            let bounds = node_bounds(scene, slot);
            let split = node_split(scene, slot);
            let entry = links.then(|| node_chain(scene, derive.content, slot));
            let mut coverage = own_change(scene, slot, before.get(&slot).copied(), world);

            let Some(record) = scene.clips.at_mut(slot) else {
                continue;
            };
            let held = ChainBefore {
                bounds: record.bounds,
                follows: record.split.follows,
                root: record.split.root,
                still: record.still,
            };
            record.bounds = bounds;
            record.split = split;
            record.still = whole_pixels(split.still);
            if let Some(entry) = entry {
                derive.tables.clips.write(record.links.frame, entry);
            }

            // Parents come first, so a changed parent's chain is already known.
            let parent = record.node.parent.map(|parent| parent.slot());
            if let Some(above) = parent.and_then(|parent| changes.coverage.get(&parent)) {
                coverage.add(above);
            }
            changes.changed.insert(slot, cause);
            changes.coverage.insert(slot, coverage);
            changes.before.insert(slot, held);
            stack.extend(record.children.iter().map(|&child| (child, cause, links)));
        }
    }

    changes
}

/// Returns how many ancestors a clip node has.
fn depth(scene: &Scene, slot: u32) -> usize {
    let mut depth = 0;
    let mut current = slot;
    while let Some(parent) = scene
        .clips
        .at(current)
        .and_then(|record| record.node.parent)
    {
        depth += 1;
        current = parent.slot();
        if depth > scene.clips.span() {
            break;
        }
    }
    depth
}
