//! An id of any kind.

use crate::animation::Animation;
use crate::id::kind::sealed::Sealed;
use crate::id::{
    AnimationId, ClipId, EffectId, Id, IdKind, ImageId, OrderId, PaintId, ParamsId, ScrollId,
    ShaderId, TextureId, TransformId,
};
use crate::node::{ClipNode, EffectNode, OrderNode, ScrollNode, TransformNode};
use crate::paint::{Paint, ParamsBlock};
use crate::resource::{ImageDesc, ShaderDesc, TextureDesc};

/// An id of any kind a client mints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnyId {
    /// A transform node.
    Transform(TransformId),
    /// A scroll node.
    Scroll(ScrollId),
    /// A clip node.
    Clip(ClipId),
    /// An effect node.
    Effect(EffectId),
    /// An order node.
    Order(OrderId),
    /// A paint slot.
    Paint(PaintId),
    /// A shader parameter slot.
    Params(ParamsId),
    /// An image.
    Image(ImageId),
    /// An external texture slot.
    Texture(TextureId),
    /// An application shader.
    Shader(ShaderId),
    /// An animation.
    Animation(AnimationId),
}

impl AnyId {
    pub(crate) const fn parts(self) -> (u32, u32) {
        match self {
            Self::Transform(id) => (id.slot(), id.generation()),
            Self::Scroll(id) => (id.slot(), id.generation()),
            Self::Clip(id) => (id.slot(), id.generation()),
            Self::Effect(id) => (id.slot(), id.generation()),
            Self::Order(id) => (id.slot(), id.generation()),
            Self::Paint(id) => (id.slot(), id.generation()),
            Self::Params(id) => (id.slot(), id.generation()),
            Self::Image(id) => (id.slot(), id.generation()),
            Self::Texture(id) => (id.slot(), id.generation()),
            Self::Shader(id) => (id.slot(), id.generation()),
            Self::Animation(id) => (id.slot(), id.generation()),
        }
    }

    pub(crate) const fn kind(self) -> usize {
        match self {
            Self::Transform(_) => TransformNode::KIND,
            Self::Scroll(_) => ScrollNode::KIND,
            Self::Clip(_) => ClipNode::KIND,
            Self::Effect(_) => EffectNode::KIND,
            Self::Order(_) => OrderNode::KIND,
            Self::Paint(_) => Paint::KIND,
            Self::Params(_) => ParamsBlock::KIND,
            Self::Image(_) => ImageDesc::KIND,
            Self::Texture(_) => TextureDesc::KIND,
            Self::Shader(_) => ShaderDesc::KIND,
            Self::Animation(_) => Animation::KIND,
        }
    }
}

impl<T: IdKind> From<Id<T>> for AnyId {
    fn from(id: Id<T>) -> Self {
        T::any(id)
    }
}
