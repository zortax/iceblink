//! Damage of what is drawn through the clips that mask with a written image.

use iceblink::{ChunkKey, ClipShape, DeviceRect, LocalParent, RecordedChunk};
use smallvec::SmallVec;

use crate::derive::Derive;
use crate::derive::order::exact_ink;
use crate::derive::route::{Routes, damage};
use crate::geometry::ink::local_ink;
use crate::scene::Scene;
use crate::scene::kind::{Kind, ObjectRef};

/// Returns the clips that mask with an image, and every clip whose chain holds one.
pub(super) fn masked_clips(scene: &Scene, masks: &[u32]) -> Vec<u32> {
    let mut clips = masks.to_vec();
    let mut index = 0;
    while index < clips.len() {
        if let Some(record) = scene.clips.at(clips[index]) {
            clips.extend(record.children.iter().copied());
        }
        index += 1;
    }
    clips
}

/// Damages every item drawn through a chunk-local clip whose chain of local clips masks with
/// image `image`.
pub(super) fn damage_local(derive: &mut Derive<'_>, image: u32, routes: &Routes) {
    let Some(links) = derive.scene.links(ObjectRef::new(Kind::Image, image)) else {
        return;
    };
    let users: SmallVec<[ChunkKey; 8]> = links.users.iter().copied().collect();
    for key in users {
        let Some(record) = derive.scene.chunks.get(&key) else {
            continue;
        };
        let chunk = &record.recorded;
        let masked = masked_locals(chunk, image);
        if !masked.contains(&true) {
            continue;
        }

        let route = routes.of_chunk(derive.scene, key);
        let inks = derive.scene.device_inks(&record.derived.inks);
        let mut rects: SmallVec<[DeviceRect; 8]> = SmallVec::new();
        for (position, &op) in chunk.ops.iter().enumerate() {
            let through = local_ink(chunk, op, &record.derived.runs)
                .and_then(|ink| ink.clip.local_index())
                .is_some_and(|index| masked.get(index as usize).copied().unwrap_or(false));
            if through {
                rects.extend(inks.item(position));
            }
        }
        for rect in rects {
            damage(derive.outputs, route, Some(rect));
        }
    }
}

/// Returns whether the chain of local clips of each chunk-local clip of `chunk` masks with image
/// `image`.
fn masked_locals(chunk: &RecordedChunk, image: u32) -> SmallVec<[bool; 4]> {
    let mut masked: SmallVec<[bool; 4]> = SmallVec::new();
    for clip in &chunk.local_clips {
        let own = matches!(clip.shape, ClipShape::Mask { image: id, .. } if id.slot() == image);
        // Parents are earlier clips, so their answer is known.
        let parent = match clip.parent {
            LocalParent::Local(index) => masked.get(index as usize).copied().unwrap_or(false),
            LocalParent::Chunk | LocalParent::Node(_) | LocalParent::None => false,
        };
        masked.push(own || parent);
    }
    masked
}

/// Damages the ink of every order node whose effect has one of `clips` as its clip.
pub(super) fn damage_effects(derive: &mut Derive<'_>, clips: &[u32], routes: &Routes) {
    let mut nodes: SmallVec<[u32; 4]> = SmallVec::new();
    for &clip in clips {
        let Some(record) = derive.scene.clips.at(clip) else {
            continue;
        };
        for &effect in &record.effects {
            if let Some(effect) = derive.scene.effects.at(effect) {
                nodes.extend(effect.orders.iter().copied());
            }
        }
    }
    nodes.sort_unstable();
    nodes.dedup();

    for node in nodes {
        let ink = exact_ink(derive.scene, node);
        let route = routes.of_group(derive.scene, node);
        damage(derive.outputs, route, ink);
    }
}
