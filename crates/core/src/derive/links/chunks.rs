//! The user sets of the objects the chunks a commit changed refer to.

use crate::commit::validate::ChunkNet;
use crate::derive::Derive;
use crate::derive::chunks::forget_loose;
use crate::derive::links::refers_to;
use crate::scene::Scene;
use crate::scene::kind::{Kind, ObjectRef};
use crate::scene::reaches::Reaches;

/// Returns the reaches of the users of `object`, if it is a clip node that keeps them.
fn reaches_of(scene: &mut Scene, object: ObjectRef) -> Option<&mut Reaches> {
    if !scene.reaches_kept {
        return None;
    }
    match object.kind {
        Kind::Clip => scene.clips.at_mut(object.slot)?.reaches.as_mut(),
        _ => None,
    }
}

/// Moves every changed chunk from the user sets of the objects it referred to before into those
/// of the objects it refers to after, and stores what it refers to.
///
/// The user sets of the `replaced` objects start empty, so every chunk that refers to one is
/// added to it.
pub(crate) fn chunks(derive: &mut Derive<'_>, nets: &[ChunkNet], replaced: &[ObjectRef]) {
    let scene = &mut *derive.scene;
    for net in nets {
        if let (Some(after), Some(record)) = (&net.after, scene.chunks.get_mut(&net.key)) {
            record.derived.deps.clone_from(after);
        }
        let fresh = net
            .after
            .as_ref()
            .is_some_and(|after| refers_to(after, replaced));
        if net.before == net.after && !fresh {
            continue;
        }

        for &object in net.before.iter().flatten() {
            if let Some(links) = scene.links_mut(object) {
                links.users.remove(&net.key);
            }
            if let Some(reaches) = reaches_of(scene, object) {
                reaches.remove(net.key);
            }
        }
        if let Some(before) = &net.before {
            forget_loose(scene, net.key, before);
        }

        for &object in net.after.iter().flatten() {
            if let Some(links) = scene.links_mut(object) {
                links.users.insert(net.key);
            }
            if let Some(reaches) = reaches_of(scene, object) {
                reaches.insert(net.key);
            }
        }
    }
}
