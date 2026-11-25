//! The objects a node's value refers to.

use iceblink::{
    Anchor, ClipNode, ClipShape, EffectNode, Filter, Id, OrderNode, Paint, ParamsBlock, ScrollNode,
    TransformNode,
};

use crate::scene::kind::{Kind, ObjectRef};

/// A stored value that can refer to other objects.
pub(crate) trait Names {
    /// Calls `name` with every object the value refers to and the generation of its id, once per
    /// mention.
    fn names(&self, name: &mut dyn FnMut(ObjectRef, u32));
}

impl Names for TransformNode {
    fn names(&self, name: &mut dyn FnMut(ObjectRef, u32)) {
        if let Some(parent) = self.parent {
            mention(name, Kind::Transform, parent);
        }
        if let Some(scroll) = self.scroll {
            mention(name, Kind::Scroll, scroll);
        }
        match self.anchor {
            Anchor::Sticky(sticky) => mention(name, Kind::Scroll, sticky.scroll),
            Anchor::Thumb(thumb) => mention(name, Kind::Scroll, thumb.scroll),
            Anchor::Scrolls | Anchor::Fixed => {}
        }
    }
}

impl Names for ScrollNode {
    fn names(&self, name: &mut dyn FnMut(ObjectRef, u32)) {
        mention(name, Kind::Transform, self.space);
    }
}

impl Names for ClipNode {
    fn names(&self, name: &mut dyn FnMut(ObjectRef, u32)) {
        if let Some(parent) = self.parent {
            mention(name, Kind::Clip, parent);
        }
        mention(name, Kind::Transform, self.space);
        if let ClipShape::Mask { image, .. } = self.shape {
            mention(name, Kind::Image, image);
        }
    }
}

impl Names for EffectNode {
    fn names(&self, name: &mut dyn FnMut(ObjectRef, u32)) {
        if let Some(clip) = self.clip {
            mention(name, Kind::Clip, clip);
        }
        filter_names(&self.filters, name);
    }
}

impl Names for OrderNode {
    fn names(&self, name: &mut dyn FnMut(ObjectRef, u32)) {
        if let Some(effect) = self.effect {
            mention(name, Kind::Effect, effect);
        }
    }
}

impl Names for Paint {
    fn names(&self, _: &mut dyn FnMut(ObjectRef, u32)) {}
}

impl Names for ParamsBlock {
    fn names(&self, _: &mut dyn FnMut(ObjectRef, u32)) {}
}

impl Names for () {
    fn names(&self, _: &mut dyn FnMut(ObjectRef, u32)) {}
}

/// Calls `name` with the shader and parameter block of every shader filter of a chain.
pub(crate) fn filter_names(filters: &[Filter], name: &mut dyn FnMut(ObjectRef, u32)) {
    for filter in filters {
        if let Filter::Shader { shader, params, .. } = filter {
            mention(name, Kind::Shader, *shader);
            mention(name, Kind::Params, *params);
        }
    }
}

/// Calls `name` with the object of `kind` that `id` names and the id's generation.
fn mention<T>(name: &mut dyn FnMut(ObjectRef, u32), kind: Kind, id: Id<T>) {
    name(ObjectRef::new(kind, id.slot()), id.generation());
}
