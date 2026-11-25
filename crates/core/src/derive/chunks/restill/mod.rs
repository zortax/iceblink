//! Carried chunks whose still pixels a clip write changed, and nothing else.
//!
//! A scroll node carries a chunk's inks in its content space, cut only by the clip links that
//! move with it. The links that move with no scroll offset cut the content to the chunk's still
//! pixels. A write to such a link, such as a scroll port that changes size, changes only those
//! still pixels, so the carried inks are kept. A carry that refers to the clip node whose chain
//! decides the still pixels follows the write by itself, and only the damage is measured. Any
//! other carry is measured again.

mod admit;
mod change;

pub(super) use self::admit::{Admitted, admit, admit_carried};
pub(super) use self::change::LastChange;

use iceblink::{ChunkKey, ClipRef, DeviceRect};
use smallvec::SmallVec;

use crate::derive::Derive;
use crate::derive::carry::Stills;
use crate::derive::chunks::Changes;
use crate::derive::chunks::locals::rewrite_locals;
use crate::derive::chunks::one::Pass;
use crate::derive::chunks::reaches::store_reaches;
use crate::derive::clips::ClipChanges;
use crate::derive::route::damage;
use crate::geometry::bounds::of_pixels;
use crate::geometry::coverage::CoverageChange;
use crate::geometry::ink::local_ink;
use crate::geometry::split::{item_split, local_splits};
use crate::scene::Scene;
use crate::scene::chunk::ChunkRecord;
use crate::scene::follows::Follows;
use crate::scene::inks::{Carry, Still, moved};
use crate::scene::kind::Kind;

/// The outcome of measuring a carried chunk's still pixels again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Restilled {
    /// The same pixels, drawn with the same carry.
    Same,
    /// A kept carry whose clip node's still pixels moved. The device inks of the content that the
    /// scroll node at this slot carries move with them.
    Moved(u32),
    /// A changed carry, which the order nodes holding the chunk take.
    Recarried,
}

/// Measures the still pixels of carried chunk `key` again and damages where they changed.
///
/// A carry that refers to a clip node stays valid while the still roots of the chunk's clips stay
/// where they were, and the node's still pixels stay whole. Any other carry is measured again.
///
/// Returns `None`, changing nothing, in these cases:
///
/// - The chunk does not exist or is not carried.
/// - The chunk's items no longer share one box of still pixels.
/// - The clip chain of one of the chunk's items follows a scroll offset other than the one of the
///   chunk's scroll node.
pub(super) fn restill(
    derive: &mut Derive<'_>,
    key: ChunkKey,
    changes: &Changes<'_>,
    pass: &mut Pass<'_>,
    last: &mut LastChange,
) -> Option<Restilled> {
    let scene = &*derive.scene;
    let record = scene.chunks.get(&key)?;
    let old = record.derived.inks.carry()?;
    let travel = scene.travel(old.scroll);
    let before = changes.clips.still_before(scene, old.still);
    if let Still::Node(root) = old.still
        && roots_hold(scene, record, changes.clips)
        && let Some(now) = scene.clips.at(root).and_then(|clip| clip.still)
    {
        // Still pixels that stay can be cut to a new shape, as by new corner radii.
        let inks = &record.derived.inks;
        let change = last.change_of(record, changes.clips);
        // Content that misses the change has no pixel it could damage.
        let content = inks
            .content()
            .map(|content| of_pixels(moved(content, travel)));
        if content.is_some_and(|content| change.meets(content)) {
            let after = (now != before).then(|| inks.at(travel, now).union());
            let cut = cut_to(change, inks.at(travel, before).union(), after.flatten());
            damage_cut(derive, key, pass, cut);
        }
        return Some(if now != before {
            Restilled::Moved(old.scroll)
        } else {
            Restilled::Same
        });
    }

    let carry = still_of(scene, key, old)?;
    if carry == old {
        return Some(Restilled::Same);
    }

    let before = record.derived.inks.at(travel, before).union();
    let change = last.change_of(record, changes.clips).clone();
    let has_locals = !record.derived.local_frames.is_empty();
    let record = derive.scene.chunks.get_mut(&key)?;
    record.derived.inks.set_carry(carry);
    store_reaches(derive.scene, key);

    let after = derive
        .scene
        .device_inks(&derive.scene.chunks[&key].derived.inks)
        .union();
    let cut = cut_to(&change, before, after);
    damage_cut(derive, key, pass, cut);
    if has_locals {
        rewrite_locals(derive, key);
    }
    derive.work.carries_replaced += 1;
    Some(Restilled::Recarried)
}

/// Reports whether every changed clip chunk `record` refers to keeps the link that decides its
/// still bounds.
fn roots_hold(scene: &Scene, record: &ChunkRecord, clips: &ClipChanges) -> bool {
    record.derived.deps.iter().all(|object| {
        object.kind != Kind::Clip
            || clips.before.get(&object.slot).is_none_or(|before| {
                scene
                    .clips
                    .at(object.slot)
                    .is_some_and(|clip| clip.split.root == before.root)
            })
    })
}

/// Returns the parts of the inks `before` and `after` inside `change`.
fn cut_to(
    change: &CoverageChange,
    before: Option<DeviceRect>,
    after: Option<DeviceRect>,
) -> SmallVec<[DeviceRect; 4]> {
    let mut cut = SmallVec::new();
    for ink in before.into_iter().chain(after) {
        cut.extend(change.cut(ink));
    }
    cut
}

/// Damages the rectangles `cut` on the output that draws chunk `key`.
fn damage_cut(
    derive: &mut Derive<'_>,
    key: ChunkKey,
    pass: &mut Pass<'_>,
    cut: SmallVec<[DeviceRect; 4]>,
) {
    if cut.is_empty() {
        return;
    }
    let route = pass.found.of_chunk(pass.routes, derive.scene, key);
    for rect in cut {
        damage(derive.outputs, route, Some(rect));
    }
}

/// Returns the carry of chunk `key` with its still pixels measured again, or `None` when its
/// items no longer share one box of still pixels or follow another scroll node.
fn still_of(scene: &Scene, key: ChunkKey, old: Carry) -> Option<Carry> {
    let record = scene.chunks.get(&key)?;
    let chunk = &record.recorded;
    let locals = local_splits(scene, chunk, record.offset);
    // Items mostly share a handful of clip references, so each one's split is taken in once.
    // Taking a split in again changes nothing.
    let mut seen: SmallVec<[ClipRef; 4]> = SmallVec::new();
    let mut stills = Stills::default();

    for &op in &chunk.ops {
        let Some(local) = local_ink(chunk, op, &record.derived.runs) else {
            continue;
        };
        if seen.contains(&local.clip) {
            continue;
        }
        seen.push(local.clip);
        let split = item_split(scene, chunk, &locals, local.clip);
        if !matches!(split.follows, Follows::Still) && split.follows != Follows::Scroll(old.scroll)
        {
            return None;
        }
        stills.add(&split)?;
    }
    Some(Carry {
        scroll: old.scroll,
        still: stills.finish(scene)?,
    })
}
