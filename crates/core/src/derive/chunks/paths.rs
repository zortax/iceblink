//! Forgetting the route histories of the path items a commit removed.

use iceblink::{ItemKind, RecordedChunk};

use crate::commit::journal::{ChunkChange, Journal};
use crate::derive::Derive;

/// Forgets the route history of every path item the commit removed.
///
/// A path is removed when its chunk is gone after the commit, or holds no path item at the
/// path's position.
pub(crate) fn forget_removed_paths(derive: &mut Derive<'_>, journal: &Journal) {
    let client = derive.scene.client;
    for change in &journal.chunks {
        let ChunkChange::Replaced {
            key,
            before: Some(index),
            ..
        } = *change
        else {
            continue;
        };
        let Some(Some(old)) = journal.replaced.get(index as usize) else {
            continue;
        };
        if old.recorded.paths.is_empty() {
            continue;
        }

        let now = derive.scene.chunks.get(&key).map(|record| &record.recorded);
        for (position, op) in old.recorded.ops.iter().enumerate() {
            if op.kind == ItemKind::PATH && !holds_path(now, position) {
                derive
                    .content
                    .cache
                    .forget_path(client, key, position as u32);
            }
        }
    }
}

/// Reports whether `chunk` holds a path item at `position`.
fn holds_path(chunk: Option<&RecordedChunk>, position: usize) -> bool {
    chunk
        .and_then(|chunk| chunk.ops.get(position))
        .is_some_and(|op| op.kind == ItemKind::PATH)
}
