//! Typed ids, the handles of clients, outputs and frames, and client-side minting.

mod any;
mod handle;
mod kind;
mod space;

use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

use crate::animation::Animation;
use crate::node::{ClipNode, EffectNode, OrderNode, ScrollNode, TransformNode};
use crate::paint::{Paint, ParamsBlock};
use crate::resource::{ImageDesc, ShaderDesc, TextureDesc};

pub use self::any::AnyId;
pub use self::handle::{ClientId, FrameId, OutputId};
pub use self::kind::IdKind;
pub use self::space::IdSpace;

/// A typed id of an object a client creates.
///
/// An id is an index and a generation. The index is the object's slot in the client's table for
/// kind `T`, and the generation tells apart the successive objects that slot has held. Minted
/// generations start at 1, so an id with generation 0 is never valid.
pub struct Id<T> {
    index: u32,
    generation: u32,
    _kind: PhantomData<fn() -> T>,
}

/// The id of a transform node.
pub type TransformId = Id<TransformNode>;
/// The id of a scroll node.
pub type ScrollId = Id<ScrollNode>;
/// The id of a clip node.
pub type ClipId = Id<ClipNode>;
/// The id of an effect node.
pub type EffectId = Id<EffectNode>;
/// The id of an order node.
pub type OrderId = Id<OrderNode>;
/// The id of a paint slot.
pub type PaintId = Id<Paint>;
/// The id of a shader parameter slot.
pub type ParamsId = Id<ParamsBlock>;
/// The id of an image.
pub type ImageId = Id<ImageDesc>;
/// The id of an external texture slot.
pub type TextureId = Id<TextureDesc>;
/// The id of an application shader.
pub type ShaderId = Id<ShaderDesc>;
/// The id of a running animation.
pub type AnimationId = Id<Animation>;

impl<T> Id<T> {
    pub(crate) const fn new(index: u32, generation: u32) -> Self {
        Self {
            index,
            generation,
            _kind: PhantomData,
        }
    }

    /// Returns the slot an item stores for this id.
    pub const fn slot(self) -> u32 {
        self.index
    }

    /// Returns the generation of the slot this id was minted with.
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<T> Eq for Id<T> {}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.index, self.generation).cmp(&(other.index, other.generation))
    }
}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({}v{})", self.index, self.generation)
    }
}
