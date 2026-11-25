//! The objects a commit replaced.

use smallvec::SmallVec;

use crate::commit::journal::{Change, Journal, Op};
use crate::commit::net::nets;
use crate::scene::Scene;
use crate::scene::kind::{Kind, ObjectRef};
use crate::scene::slab::Slab;
use crate::scene::stored::Stored;

/// Returns every object the commit removed and then created again at the same slot.
pub(crate) fn replaced(scene: &Scene, journal: &Journal) -> SmallVec<[ObjectRef; 2]> {
    let mut replaced = SmallVec::new();
    // Only a commit that removed an object can replace one.
    if journal.freed.is_empty() {
        return replaced;
    }

    of(
        &journal.transforms,
        &scene.transforms,
        Kind::Transform,
        &mut replaced,
    );
    of(
        &journal.scrolls,
        &scene.scrolls,
        Kind::Scroll,
        &mut replaced,
    );
    of(&journal.clips, &scene.clips, Kind::Clip, &mut replaced);
    of(
        &journal.effects,
        &scene.effects,
        Kind::Effect,
        &mut replaced,
    );
    of(&journal.paints, &scene.paints, Kind::Paint, &mut replaced);
    of(&journal.params, &scene.params, Kind::Params, &mut replaced);
    of(&journal.images, &scene.images, Kind::Image, &mut replaced);
    of(
        &journal.textures,
        &scene.textures,
        Kind::Texture,
        &mut replaced,
    );
    of(
        &journal.shaders,
        &scene.shaders,
        Kind::Shader,
        &mut replaced,
    );
    replaced
}

/// Reports whether any of `objects` is one of the `replaced` objects.
pub(crate) fn refers_to(objects: &[ObjectRef], replaced: &[ObjectRef]) -> bool {
    !replaced.is_empty() && objects.iter().any(|object| replaced.contains(object))
}

/// Adds the objects of `kind` that `changes` replaced to `replaced`.
fn of<R: Stored>(
    changes: &[Change<R>],
    slab: &Slab<R>,
    kind: Kind,
    replaced: &mut SmallVec<[ObjectRef; 2]>,
) {
    // A commit that did not both remove and create an object of the kind replaced none.
    let removes = changes
        .iter()
        .any(|change| matches!(change.op, Op::Removed(_)));
    if !removes
        || !changes
            .iter()
            .any(|change| matches!(change.op, Op::Created(_)))
    {
        return;
    }

    let found = nets(changes, slab);
    let found = found.iter().filter(|net| net.replaced());
    replaced.extend(found.map(|net| ObjectRef::new(kind, net.slot)));
}
