//! The frame clip slots of chunk-local clips.

use iceblink::ChunkKey;
use smallvec::SmallVec;

use crate::derive::Derive;
use crate::tables::clip::local_chain;

/// Gives every chunk-local clip of a newly recorded chunk a frame slot.
pub(super) fn allocate_locals(derive: &mut Derive<'_>, key: ChunkKey) {
    let Some(record) = derive.scene.chunks.get(&key) else {
        return;
    };
    let entries: SmallVec<[_; 2]> = (0..record.recorded.local_clips.len())
        .map(|index| {
            local_chain(
                derive.scene,
                derive.content,
                &record.recorded,
                index,
                record.offset,
            )
        })
        .collect();
    let frames: SmallVec<[u32; 2]> = entries
        .into_iter()
        .map(|entry| derive.tables.clips.allocate(entry))
        .collect();
    if let Some(record) = derive.scene.chunks.get_mut(&key) {
        record.derived.local_frames = frames;
    }
}

/// Rewrites the frame entries of a chunk's local clips, for a new offset, props or parent chain.
pub(super) fn rewrite_locals(derive: &mut Derive<'_>, key: ChunkKey) {
    let Some(record) = derive.scene.chunks.get(&key) else {
        return;
    };
    for (index, &frame) in record.derived.local_frames.iter().enumerate() {
        let entry = local_chain(
            derive.scene,
            derive.content,
            &record.recorded,
            index,
            record.offset,
        );
        derive.tables.clips.write(frame, entry);
    }
}
