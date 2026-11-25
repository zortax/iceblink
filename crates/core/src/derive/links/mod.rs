//! Reference counts, reverse indices and frame slots of the nodes, slots and chunks a commit
//! changed.

mod chunks;
mod clips;
mod orders;
mod replaced;
mod slots;
mod spaces;

pub(crate) use self::chunks::chunks;
pub(crate) use self::orders::unwatch;
pub(crate) use self::replaced::{refers_to, replaced};

use iceblink::ClipNode;

use crate::commit::journal::Journal;
use crate::derive::Derive;
use crate::scene::Scene;
use crate::scene::names::Names;

/// Why a transform node's world matrix is recomputed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cause {
    /// The node or an ancestor was created or written.
    Write,
    /// The offset of the scroll node at this slot changed.
    Scroll(u32),
}

/// The transform and clip nodes whose derived state the commit made stale.
#[derive(Debug, Default)]
pub(crate) struct Moved {
    /// The transform nodes whose world matrix is recomputed, with why.
    pub(crate) transforms: Vec<(u32, Cause)>,
    /// The clip nodes the commit created or wrote, with their value before the commit, or `None`
    /// for a node it created.
    pub(crate) clips: Vec<(u32, Option<ClipNode>)>,
}

/// Updates the reference counts, indices and frame slots of every node, slot and resource the
/// commit changed, and returns the transform and clip nodes to recompute.
pub(crate) fn nodes(derive: &mut Derive<'_>, journal: &Journal) -> Moved {
    let mut moved = Moved::default();
    // A kind the commit did not change is not looked at, so its code is not run.
    if !journal.transforms.is_empty() {
        spaces::transforms(derive, journal, &mut moved);
    }
    if !journal.scrolls.is_empty() {
        spaces::scrolls(derive, journal, &mut moved);
    }
    if !journal.clips.is_empty() {
        clips::clips(derive, journal, &mut moved);
    }
    if !journal.effects.is_empty() {
        orders::effects(derive, journal);
    }
    if !journal.orders.is_empty() {
        orders::orders(derive, journal);
    }
    if !journal.paints.is_empty() {
        slots::paints(derive, journal);
    }
    if !journal.params.is_empty() {
        slots::params(derive, journal);
    }
    if !journal.shaders.is_empty() {
        slots::shaders(derive, journal);
    }
    moved
}

/// Moves one reference from every object `before` refers to to every object `after` refers to.
///
/// A reference to an object the commit removed is dropped with it, so it moves from nothing.
pub(super) fn renames<V: Names>(scene: &mut Scene, before: Option<&V>, after: Option<&V>) {
    if let Some(before) = before {
        before.names(&mut |object, generation| {
            if scene.holds(object, generation)
                && let Some(links) = scene.links_mut(object)
            {
                links.refs = links.refs.saturating_sub(1);
            }
        });
    }
    if let Some(after) = after {
        after.names(&mut |object, generation| {
            if scene.holds(object, generation)
                && let Some(links) = scene.links_mut(object)
            {
                links.refs += 1;
            }
        });
    }
}

/// Removes one occurrence of `slot` from an index list, if the list exists.
pub(super) fn remove_from(list: &mut Option<&mut Vec<u32>>, slot: u32) {
    if let Some(list) = list
        && let Some(index) = list.iter().position(|&held| held == slot)
    {
        list.swap_remove(index);
    }
}
