//! Which carried chunks a clip write changes only the still pixels of.

use smallvec::SmallVec;

use crate::derive::chunks::Changes;
use crate::derive::chunks::restill::Restilled;
use crate::derive::chunks::restill::change::LastChange;
use crate::geometry::bounds::of_pixels;
use crate::geometry::coverage::CoverageChange;
use crate::scene::Scene;
use crate::scene::chunk::ChunkRecord;
use crate::scene::follows::Follows;
use crate::scene::inks::{Still, moved};
use crate::scene::kind::Kind;
use crate::scene::reaches::CarriedReach;

/// How the clip writes of a commit change a chunk the commit did not write.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::derive::chunks) enum Admitted {
    /// A chunk that is not restilled.
    No,
    /// A chunk restilled with no damage, for which [`restill`] would report this outcome.
    ///
    /// [`restill`]: super::restill
    Quiet(Restilled),
    /// A chunk to be restilled one by one.
    Restill,
}

/// Returns how the clip writes change chunk `record`, which the commit did not write.
///
/// The chunk is restilled when all of these hold:
///
/// 1. It is carried and holds no backdrop that reads around it.
/// 2. No space it refers to moved.
/// 3. Every changed clip it refers to moves with no scroll offset before and after the commit.
///
/// It is restilled quietly when also its carry refers to a clip node whose still pixels stay
/// whole, the still roots of its changed clips stay where they were, and its content misses
/// where those clips cover differently.
pub(in crate::derive::chunks) fn admit(
    scene: &Scene,
    record: &ChunkRecord,
    changes: &Changes<'_>,
    last: &mut LastChange,
) -> Admitted {
    let inks = &record.derived.inks;
    let Some(carry) = inks.carry() else {
        return Admitted::No;
    };
    if record.derived.reads_around {
        return Admitted::No;
    }

    let mut roots_hold = true;
    let mut changed: SmallVec<[u32; 4]> = SmallVec::new();
    for object in &record.derived.deps {
        match object.kind {
            Kind::Transform if changes.world.moved.contains_key(&object.slot) => {
                return Admitted::No;
            }
            Kind::Clip => {
                let Some(before) = changes.clips.before.get(&object.slot) else {
                    continue;
                };
                let Some(clip) = scene.clips.at(object.slot) else {
                    return Admitted::No;
                };
                if before.follows != Follows::Still || clip.split.follows != Follows::Still {
                    return Admitted::No;
                }
                roots_hold &= clip.split.root == before.root;
                // A changed clip has a coverage change as well.
                changed.push(object.slot);
            }
            _ => {}
        }
    }

    let Still::Node(root) = carry.still else {
        return Admitted::Restill;
    };
    let Some(moved_still) = roots_hold
        .then(|| last.still_moved(root, scene, changes.clips))
        .flatten()
    else {
        return Admitted::Restill;
    };
    let change = last.change(&changed, changes.clips);
    let travel = scene.travel(carry.scroll);
    let content = inks
        .content()
        .map(|content| of_pixels(moved(content, travel)));
    if content.is_some_and(|content| change.meets(content)) {
        return Admitted::Restill;
    }
    Admitted::Quiet(if moved_still {
        Restilled::Moved(carry.scroll)
    } else {
        Restilled::Same
    })
}

/// Returns how a write of a clip node that covers differently in `change` restills its carried
/// user `carried` quietly, or `None` when the user must be checked one by one.
///
/// The clip must have changed, moved with no scroll offset before and after the commit, and kept
/// its still root. The user must refer to no space a travel moved and be in no motion of a space.
/// The clip is then the user's only clip, so the user restills as [`admit`] decides: quietly when
/// its content misses `change` and the still pixels of its carry's clip node stay whole.
// Inlined into the walk over every carried user of a written clip.
#[inline]
pub(in crate::derive::chunks) fn admit_carried(
    scene: &Scene,
    change: &CoverageChange,
    carried: CarriedReach,
    changes: &Changes<'_>,
    last: &mut LastChange,
) -> Option<Restilled> {
    let travel = scene.travel(carried.scroll);
    let content = carried
        .content
        .map(|content| of_pixels(moved(content, travel)));
    if content.is_some_and(|content| change.meets(content)) {
        return None;
    }
    let moved = last.still_moved(carried.root, scene, changes.clips)?;
    Some(if moved {
        Restilled::Moved(carried.scroll)
    } else {
        Restilled::Same
    })
}
