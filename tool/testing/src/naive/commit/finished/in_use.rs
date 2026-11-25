//! Objects a transaction removed while something still refers to them.

use std::collections::HashSet;

use iceblink::{Anchor, AnyId, ClipShape, Filter, Id, LocalParent, OrderEntry, RejectReason};

use crate::naive::commit::draft::Draft;
use crate::naive::commit::finished::references::{self, Slot};
use crate::naive::commit::precedence::Earliest;
use crate::naive::scene::Client;

/// The kind and slot of an object something refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Object {
    Transform(u32),
    Scroll(u32),
    Clip(u32),
    Effect(u32),
    Order(u32),
    Paint(u32),
    Params(u32),
    Image(u32),
    Texture(u32),
    Shader(u32),
}

/// An object something refers to, with the generation of the id that names it, or `None` for a
/// reference outside the props of a chunk the transaction did not record, which refers to any
/// object removed at the slot.
type Reference = (Object, Option<u32>);

/// Notes `InUse` at the first removal of an object the client still refers to.
///
/// References come from the client's nodes, order entries and output roots, and from the chunks
/// the transaction did not record.
pub(super) fn check(draft: &Draft<'_>, earliest: &mut Earliest) {
    if draft.removed.is_empty() {
        return;
    }

    let referenced = referenced(draft);
    let found = draft.removed.iter().find(|(id, _)| {
        object(*id).is_some_and(|(object, generation)| {
            referenced.contains(&(object, Some(generation))) || referenced.contains(&(object, None))
        })
    });
    if let Some((_, index)) = found {
        earliest.note(*index, RejectReason::InUse);
    }
}

/// Returns the object `id` refers to and the id's generation, or `None` for an animation, which
/// no state refers to.
fn object(id: AnyId) -> Option<(Object, u32)> {
    Some(match id {
        AnyId::Transform(id) => (Object::Transform(id.slot()), id.generation()),
        AnyId::Scroll(id) => (Object::Scroll(id.slot()), id.generation()),
        AnyId::Clip(id) => (Object::Clip(id.slot()), id.generation()),
        AnyId::Effect(id) => (Object::Effect(id.slot()), id.generation()),
        AnyId::Order(id) => (Object::Order(id.slot()), id.generation()),
        AnyId::Paint(id) => (Object::Paint(id.slot()), id.generation()),
        AnyId::Params(id) => (Object::Params(id.slot()), id.generation()),
        AnyId::Image(id) => (Object::Image(id.slot()), id.generation()),
        AnyId::Texture(id) => (Object::Texture(id.slot()), id.generation()),
        AnyId::Shader(id) => (Object::Shader(id.slot()), id.generation()),
        AnyId::Animation(_) => return None,
    })
}

/// Returns the reference of an id of the kind `object` makes from a slot.
fn named<T>(object: fn(u32) -> Object, id: Id<T>) -> Reference {
    (object(id.slot()), Some(id.generation()))
}

/// Returns every object the client's state refers to.
fn referenced(draft: &Draft<'_>) -> HashSet<Reference> {
    let client = &draft.client;
    let mut referenced = HashSet::new();
    nodes(client, &mut referenced);

    for (_, root) in draft
        .roots
        .values()
        .filter(|(owner, _)| *owner == draft.client_id)
    {
        referenced.insert(named(Object::Order, *root));
    }

    // A recorded chunk that refers to a removed object breaks the chunk's own rules.
    let kept = client
        .chunks
        .iter()
        .filter(|(key, _)| !draft.recorded.contains_key(key));
    for (_, chunk) in kept {
        referenced.insert(named(Object::Transform, chunk.props.space));
        if let Some(clip) = chunk.props.clip {
            referenced.insert(named(Object::Clip, clip));
        }

        let recorded = &chunk.recorded;
        let refs = recorded.refs.iter().filter_map(|id| object(*id));
        referenced.extend(refs.map(|(object, _)| (object, None)));

        for clip in &recorded.local_clips {
            if let LocalParent::Node(parent) = clip.parent {
                referenced.insert((Object::Clip(parent.slot()), None));
            }
            if let ClipShape::Mask { image, .. } = clip.shape {
                referenced.insert((Object::Image(image.slot()), None));
            }
        }

        references::for_each(recorded, &mut |slot| {
            let object = match slot {
                Slot::Transform(slot) => Object::Transform(slot),
                Slot::Clip(slot) => Object::Clip(slot),
                Slot::Paint(slot) => Object::Paint(slot),
                Slot::Params(slot) => Object::Params(slot),
                Slot::Shader(slot) => Object::Shader(slot),
                Slot::Image(slot) => Object::Image(slot),
                Slot::Texture(slot) => Object::Texture(slot),
            };
            referenced.insert((object, None));
        });
    }

    referenced
}

/// Adds the objects the client's nodes refer to.
fn nodes(client: &Client, referenced: &mut HashSet<Reference>) {
    for (_, _, node) in client.transforms.iter() {
        if let Some(parent) = node.parent {
            referenced.insert(named(Object::Transform, parent));
        }
        if let Some(scroll) = node.scroll {
            referenced.insert(named(Object::Scroll, scroll));
        }
        match node.anchor {
            Anchor::Sticky(sticky) => {
                referenced.insert(named(Object::Scroll, sticky.scroll));
            }
            Anchor::Thumb(thumb) => {
                referenced.insert(named(Object::Scroll, thumb.scroll));
            }
            Anchor::Scrolls | Anchor::Fixed => {}
        }
    }

    for (_, _, node) in client.scrolls.iter() {
        referenced.insert(named(Object::Transform, node.space));
    }

    for (_, _, node) in client.clips.iter() {
        referenced.insert(named(Object::Transform, node.space));
        if let Some(parent) = node.parent {
            referenced.insert(named(Object::Clip, parent));
        }
        if let ClipShape::Mask { image, .. } = node.shape {
            referenced.insert(named(Object::Image, image));
        }
    }

    for (_, _, node) in client.effects.iter() {
        if let Some(clip) = node.clip {
            referenced.insert(named(Object::Clip, clip));
        }
        for filter in node.filters.iter() {
            if let Filter::Shader { shader, params, .. } = filter {
                referenced.insert(named(Object::Shader, *shader));
                referenced.insert(named(Object::Params, *params));
            }
        }
    }

    for (_, _, order) in client.orders.iter() {
        if let Some(effect) = order.node.effect {
            referenced.insert(named(Object::Effect, effect));
        }
        for entry in &order.entries {
            if let OrderEntry::Node(child) = entry {
                referenced.insert(named(Object::Order, *child));
            }
        }
    }
}
