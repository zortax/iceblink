//! References to stored objects by kind and slot.

/// A kind of object a client creates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Kind {
    Transform,
    Scroll,
    Clip,
    Effect,
    Paint,
    Params,
    Image,
    Texture,
    Shader,
}

/// A stored object, by its kind and the slot of its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ObjectRef {
    pub(crate) kind: Kind,
    pub(crate) slot: u32,
}

impl ObjectRef {
    pub(crate) const fn new(kind: Kind, slot: u32) -> Self {
        Self { kind, slot }
    }
}
