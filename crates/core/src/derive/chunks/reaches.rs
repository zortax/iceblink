//! The reaches that clip nodes keep of their users.

use iceblink::ChunkKey;
use smallvec::SmallVec;

use crate::derive::carry::carrier;
use crate::scene::Scene;
use crate::scene::chunk::ChunkRecord;
use crate::scene::inks::{ChunkInks, Still};
use crate::scene::kind::{Kind, ObjectRef};
use crate::scene::reaches::{CarriedReach, Reach, Reaches};

/// Makes every clip node of `clips` keep the reaches of its users.
pub(super) fn keep_reaches(scene: &mut Scene, clips: &[u32]) {
    scene.reaches_kept |= !clips.is_empty();
    for &clip in clips {
        let Some(record) = scene.clips.at(clip) else {
            continue;
        };
        let mut reaches = Reaches::default();
        for &key in &record.links.users {
            match scene.chunks.get(&key) {
                Some(user) => reaches.set(reach_of(scene, key, user, &user.derived.inks)),
                None => reaches.insert(key),
            }
        }
        if let Some(record) = scene.clips.at_mut(clip) {
            record.reaches = Some(reaches);
        }
    }
}

/// Stores the reach of chunk `key` from its stored inks in every clip node it refers to that
/// keeps its users' reaches.
pub(crate) fn store_reaches(scene: &mut Scene, key: ChunkKey) {
    if !scene.reaches_kept {
        return;
    }
    let Some(record) = scene.chunks.get(&key) else {
        return;
    };
    let clips: SmallVec<[u32; 4]> = record
        .derived
        .deps
        .iter()
        .filter(|object| keeps_reaches(scene, object))
        .map(|object| object.slot)
        .collect();
    if clips.is_empty() {
        return;
    }
    let reach = reach_of(scene, key, record, &record.derived.inks);
    for slot in clips {
        set_reach(scene, slot, reach);
    }
}

/// Returns the reach of chunk `key` with `inks`, if a clip node it refers to keeps its users'
/// reaches.
pub(super) fn reach_to_store(scene: &Scene, key: ChunkKey, inks: &ChunkInks) -> Option<Reach> {
    scene
        .reaches_kept
        .then(|| scene.chunks.get(&key))
        .flatten()
        .filter(|record| {
            record
                .derived
                .deps
                .iter()
                .any(|object| keeps_reaches(scene, object))
        })
        .map(|record| reach_of(scene, key, record, inks))
}

/// Stores `reach` in the clip node at `slot`, if it keeps its users' reaches.
pub(super) fn set_reach(scene: &mut Scene, slot: u32, reach: Reach) {
    if let Some(reaches) = scene
        .clips
        .at_mut(slot)
        .and_then(|clip| clip.reaches.as_mut())
    {
        reaches.set(reach);
    }
}

/// Returns what a clip node that keeps its users' reaches stores of chunk `record` with `inks`.
pub(crate) fn reach_of(
    scene: &Scene,
    key: ChunkKey,
    record: &ChunkRecord,
    inks: &ChunkInks,
) -> Reach {
    let carry = inks.carry();
    let carried = carry.and_then(|carry| {
        let Still::Node(root) = carry.still else {
            return None;
        };
        let clips = record
            .derived
            .deps
            .iter()
            .filter(|object| object.kind == Kind::Clip)
            .count();
        (clips == 1 && !record.derived.reads_around).then_some(CarriedReach {
            content: inks.content(),
            scroll: carry.scroll,
            root,
        })
    });
    Reach {
        key,
        reach: inks.reach(),
        plain: plain(scene, record, carry.is_some()),
        carried,
    }
}

/// Reports whether `object` is a clip node that keeps its users' reaches.
fn keeps_reaches(scene: &Scene, object: &ObjectRef) -> bool {
    object.kind == Kind::Clip
        && scene
            .clips
            .at(object.slot)
            .is_some_and(|clip| clip.reaches.is_some())
}

/// Reports whether chunk `record`, carried when `carried`, is plain: on the device, without a
/// scroll node that can carry it, reading nothing around itself and without local clips.
fn plain(scene: &Scene, record: &ChunkRecord, carried: bool) -> bool {
    !carried
        && !record.derived.reads_around
        && record.derived.local_frames.is_empty()
        && carrier(scene, record).is_none()
}
