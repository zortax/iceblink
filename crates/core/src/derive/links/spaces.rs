//! The links of the transform and scroll nodes a commit changed.

use iceblink::{Anchor, Matrix4, ScrollNode, TransformNode};

use crate::commit::journal::Journal;
use crate::commit::net::nets;
use crate::derive::Derive;
use crate::derive::links::{Cause, Moved, remove_from, renames};
use crate::scene::record::NO_FRAME_SLOT;

/// Updates the child lists, scroll dependents and frame slots of the transform nodes the commit
/// changed, and adds the nodes it created or wrote to `moved`.
pub(super) fn transforms(derive: &mut Derive<'_>, journal: &Journal, moved: &mut Moved) {
    let changes: Vec<(u32, Option<TransformNode>, u32, bool)> =
        nets(&journal.transforms, &derive.scene.transforms)
            .iter()
            .map(|net| {
                (
                    net.slot,
                    net.before.copied(),
                    net.links.map_or(NO_FRAME_SLOT, |l| l.frame),
                    net.created,
                )
            })
            .collect();

    for (slot, before, frame, created) in changes {
        let scene = &mut *derive.scene;
        let after = scene.transforms.at(slot).map(|record| record.node);
        renames(scene, before.as_ref(), after.as_ref());

        if let Some(before) = before {
            if let Some(parent) = before.parent {
                remove_from(
                    &mut scene
                        .transforms
                        .at_mut(parent.slot())
                        .map(|r| &mut r.children),
                    slot,
                );
            }
            for scroll in scrolls_of(&before) {
                remove_from(
                    &mut scene.scrolls.at_mut(scroll).map(|r| &mut r.dependents),
                    slot,
                );
            }
        }

        match after {
            Some(after) => {
                if let Some(parent) = after.parent
                    && let Some(record) = scene.transforms.at_mut(parent.slot())
                {
                    record.children.push(slot);
                }
                for scroll in scrolls_of(&after) {
                    if let Some(record) = scene.scrolls.at_mut(scroll) {
                        record.dependents.push(slot);
                    }
                }
                if created {
                    if frame != NO_FRAME_SLOT {
                        derive.tables.spaces.release(frame, derive.serial);
                    }
                    let frame = derive.tables.spaces.allocate(Matrix4::IDENTITY);
                    if let Some(record) = scene.transforms.at_mut(slot) {
                        record.links.frame = frame;
                    }
                }
                moved.transforms.push((slot, Cause::Write));
            }
            None if frame != NO_FRAME_SLOT => derive.tables.spaces.release(frame, derive.serial),
            None => {}
        }
    }
}

/// Returns the scroll nodes a transform node depends on: the one it applies and the one its
/// anchor follows.
fn scrolls_of(node: &TransformNode) -> impl Iterator<Item = u32> {
    let anchored = match node.anchor {
        Anchor::Sticky(sticky) => Some(sticky.scroll.slot()),
        Anchor::Thumb(thumb) => Some(thumb.scroll.slot()),
        Anchor::Scrolls | Anchor::Fixed => None,
    };
    node.scroll.map(|id| id.slot()).into_iter().chain(anchored)
}

/// Adds the dependents of every scroll node the commit wrote to `moved`.
pub(super) fn scrolls(derive: &mut Derive<'_>, journal: &Journal, moved: &mut Moved) {
    let changes: Vec<(u32, Option<ScrollNode>)> = nets(&journal.scrolls, &derive.scene.scrolls)
        .iter()
        .map(|net| (net.slot, net.before.cloned()))
        .collect();

    for (slot, before) in changes {
        let scene = &mut *derive.scene;
        let after = scene.scrolls.at(slot).map(|record| record.node.clone());
        renames(scene, before.as_ref(), after.as_ref());

        if before.is_some()
            && after.is_some()
            && let Some(record) = scene.scrolls.at(slot)
        {
            moved.transforms.extend(
                record
                    .dependents
                    .iter()
                    .map(|&node| (node, Cause::Scroll(slot))),
            );
        }
    }
}
