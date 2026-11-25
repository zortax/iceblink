//! The users no scroll node carries of the spaces and clips that follow a scroll offset.

use iceblink::ChunkKey;

use crate::derive::chunks::reaches::{reach_to_store, set_reach};
use crate::scene::Scene;
use crate::scene::follows::Follows;
use crate::scene::inks::ChunkInks;
use crate::scene::kind::{Kind, ObjectRef};

/// Stores the inks of chunk `key`, and adds the chunk to the loose users of every space and clip
/// it refers to that follows a scroll offset, or removes it from all of them when a scroll node
/// carries it.
///
/// The clips it refers to that keep their users' reaches store what
/// [`reach_of`](super::reaches::reach_of) returns.
// Inlined, so the inks move into the record without a copy.
#[inline]
pub(crate) fn set_inks(scene: &mut Scene, key: ChunkKey, inks: ChunkInks) {
    let carried = inks.carry().is_some();
    let reach = reach_to_store(scene, key, &inks);
    let Some(record) = scene.chunks.get_mut(&key) else {
        return;
    };
    record.derived.inks = inks;
    let deps = core::mem::take(&mut record.derived.deps);
    for &object in &deps {
        if let Some(reach) = reach
            && object.kind == Kind::Clip
        {
            set_reach(scene, object.slot, reach);
        }
        let Some((follows, loose)) = loose_of(scene, object) else {
            continue;
        };
        if !carried && follows != Follows::Still {
            loose.insert(key);
        } else {
            loose.remove(&key);
        }
    }
    if let Some(record) = scene.chunks.get_mut(&key) {
        record.derived.deps = deps;
    }
}

/// Removes chunk `key` from the loose users of every object in `objects`.
pub(crate) fn forget_loose(scene: &mut Scene, key: ChunkKey, objects: &[ObjectRef]) {
    for &object in objects {
        if let Some((_, loose)) = loose_of(scene, object) {
            loose.remove(&key);
        }
    }
}

/// Returns what a space or clip follows and its loose users.
fn loose_of(
    scene: &mut Scene,
    object: ObjectRef,
) -> Option<(Follows, &mut rustc_hash::FxHashSet<ChunkKey>)> {
    match object.kind {
        Kind::Transform => scene
            .transforms
            .at_mut(object.slot)
            .map(|record| (record.follows, &mut record.loose)),
        Kind::Clip => scene
            .clips
            .at_mut(object.slot)
            .map(|record| (record.split.follows, &mut record.loose)),
        _ => None,
    }
}
