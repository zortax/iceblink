//! The links of the clip nodes a commit changed.

use iceblink::{ClipNode, ClipShape};

use crate::commit::journal::Journal;
use crate::commit::net::nets;
use crate::derive::Derive;
use crate::derive::links::{Moved, remove_from, renames};
use crate::scene::record::NO_FRAME_SLOT;
use crate::tables::clip::node_chain;

/// Updates the child lists, mask users, space clips and frame slots of the clip nodes the commit
/// changed, and adds the nodes it created or wrote to `moved`.
pub(super) fn clips(derive: &mut Derive<'_>, journal: &Journal, moved: &mut Moved) {
    let changes: Vec<(u32, Option<ClipNode>, u32, bool)> =
        nets(&journal.clips, &derive.scene.clips)
            .iter()
            .map(|net| {
                (
                    net.slot,
                    net.before.cloned(),
                    net.links.map_or(NO_FRAME_SLOT, |l| l.frame),
                    net.created,
                )
            })
            .collect();

    for (slot, before, frame, created) in changes {
        let scene = &mut *derive.scene;
        let after = scene.clips.at(slot).map(|record| record.node.clone());
        renames(scene, before.as_ref(), after.as_ref());

        if let Some(before) = &before {
            if let Some(parent) = before.parent {
                remove_from(
                    &mut scene.clips.at_mut(parent.slot()).map(|r| &mut r.children),
                    slot,
                );
            }
            if let ClipShape::Mask { image, .. } = before.shape {
                remove_from(
                    &mut scene.images.at_mut(image.slot()).map(|r| &mut r.masks),
                    slot,
                );
            }
            remove_from(
                &mut scene
                    .transforms
                    .at_mut(before.space.slot())
                    .map(|r| &mut r.clips),
                slot,
            );
        }

        match &after {
            Some(after) => {
                if let Some(parent) = after.parent
                    && let Some(record) = scene.clips.at_mut(parent.slot())
                {
                    record.children.push(slot);
                }
                if let ClipShape::Mask { image, .. } = after.shape
                    && let Some(record) = scene.images.at_mut(image.slot())
                {
                    record.masks.push(slot);
                }
                if let Some(record) = scene.transforms.at_mut(after.space.slot()) {
                    record.clips.push(slot);
                }
                if created {
                    if frame != NO_FRAME_SLOT {
                        derive.tables.clips.release(frame, derive.serial);
                    }
                    let entry = node_chain(scene, derive.content, slot);
                    let frame = derive.tables.clips.allocate(entry);
                    if let Some(record) = scene.clips.at_mut(slot) {
                        record.links.frame = frame;
                    }
                }
                moved
                    .clips
                    .push((slot, if created { None } else { before }));
            }
            None if frame != NO_FRAME_SLOT => derive.tables.clips.release(frame, derive.serial),
            None => {}
        }
    }
}
