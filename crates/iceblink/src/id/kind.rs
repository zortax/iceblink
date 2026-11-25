//! The kinds of object an `IdSpace` mints ids for.

use crate::animation::Animation;
use crate::id::{AnyId, Id};
use crate::node::{ClipNode, EffectNode, OrderNode, ScrollNode, TransformNode};
use crate::paint::{Paint, ParamsBlock};
use crate::resource::{ImageDesc, ShaderDesc, TextureDesc};

/// A kind of object a client creates, one per id alias.
///
/// The trait is sealed, so the set of kinds is the one this crate defines.
pub trait IdKind: sealed::Sealed {}

pub(crate) mod sealed {
    use crate::id::{AnyId, Id};

    /// The per-kind facts minting and removal need.
    pub trait Sealed: Sized {
        /// The kind's position in an id space's per-kind tables.
        const KIND: usize;

        /// Wraps an id of this kind.
        fn any(id: Id<Self>) -> AnyId;
    }
}

/// How many kinds there are.
pub(crate) const KIND_COUNT: usize = 11;

macro_rules! kinds {
    ($($kind:ty => $variant:ident = $index:expr),+ $(,)?) => {
        $(
            impl sealed::Sealed for $kind {
                const KIND: usize = $index;

                fn any(id: Id<Self>) -> AnyId {
                    AnyId::$variant(id)
                }
            }

            impl IdKind for $kind {}
        )+
    };
}

kinds! {
    TransformNode => Transform = 0,
    ScrollNode => Scroll = 1,
    ClipNode => Clip = 2,
    EffectNode => Effect = 3,
    OrderNode => Order = 4,
    Paint => Paint = 5,
    ParamsBlock => Params = 6,
    ImageDesc => Image = 7,
    TextureDesc => Texture = 8,
    ShaderDesc => Shader = 9,
    Animation => Animation = 10,
}
