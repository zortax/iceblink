//! Rewriting the clip entries that read mask tiles, after the content cache started over.

use iceblink::{ClientId, ClipShape};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::client::Client;
use crate::shared_content::Content;
use crate::tables::FrameTables;
use crate::tables::clip::{local_chain, node_chain};

/// Rewrites the clip entries of every mask clip node and the clip nodes below it, and the local
/// clip entries of every chunk, so they read the images' new tiles.
pub(crate) fn refresh(
    clients: &mut FxHashMap<ClientId, Client>,
    tables: &mut FrameTables,
    content: &mut Content,
) {
    for client in clients.values() {
        let scene = &client.scene;
        let mut stack = Vec::new();
        scene.clips.each(|slot, _, record| {
            if matches!(record.node.shape, ClipShape::Mask { .. }) {
                stack.push(slot);
            }
        });

        let mut seen = FxHashSet::default();
        while let Some(slot) = stack.pop() {
            if !seen.insert(slot) {
                continue;
            }
            let Some(record) = scene.clips.at(slot) else {
                continue;
            };
            stack.extend(record.children.iter().copied());

            let frame = record.links.frame;
            let entry = node_chain(scene, content, slot);
            tables.clips.write(frame, entry);
        }

        for record in scene.chunks.values() {
            for (index, &frame) in record.derived.local_frames.iter().enumerate() {
                let entry = local_chain(scene, content, &record.recorded, index, record.offset);
                tables.clips.write(frame, entry);
            }
        }
    }
}
