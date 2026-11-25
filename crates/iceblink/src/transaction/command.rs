//! The commands of a transaction.

use crate::animation::Animation;
use crate::chunk::{ChunkKey, ChunkProps};
use crate::damage::DamageSet;
use crate::geometry::Matrix4;
use crate::id::{
    AnimationId, AnyId, ClipId, EffectId, ImageId, OrderId, OutputId, PaintId, ParamsId, ScrollId,
    ShaderId, TextureId, TransformId,
};
use crate::item::ArenaRange;
use crate::node::{ClipNode, EffectNode, OrderNode, ScrollNode, TransformNode};
use crate::paint::{Paint, ParamsBlock};
use crate::resource::{ImageDesc, ImageWrite, ShaderDesc, TextureDesc};

/// One change a transaction makes.
///
/// Each variant records a call of the [`Transaction`](crate::Transaction) method with the same
/// name, together with the id that method minted or was passed.
#[derive(Clone, Debug)]
pub enum Command {
    /// To the compositor: create a transform node.
    CreateTransform(TransformId, TransformNode),
    /// To the compositor: write a transform node's local matrix.
    SetTransform(TransformId, Matrix4),
    /// To the compositor: write a whole transform node.
    SetTransformNode(TransformId, TransformNode),
    /// To the compositor: create a scroll node.
    CreateScroll(ScrollId, ScrollNode),
    /// To the compositor: write a whole scroll node.
    SetScrollNode(ScrollId, ScrollNode),
    /// To the compositor: write a scroll node's offset.
    ScrollTo(ScrollId, [f32; 2]),
    /// To the compositor: take `offset` as a scroll node's base, which includes the compositor's
    /// scrolling up to `serial`.
    AcknowledgeScroll {
        /// The node whose offset is acknowledged.
        id: ScrollId,
        /// The serial of the last scroll event the client adopted.
        serial: u64,
        /// The offset the client now writes as its base.
        offset: [f32; 2],
    },
    /// To the compositor: create a clip node.
    CreateClip(ClipId, ClipNode),
    /// To the compositor: write a clip node.
    SetClip(ClipId, ClipNode),
    /// To the compositor: create an effect node.
    CreateEffect(EffectId, EffectNode),
    /// To the compositor: write an effect node.
    SetEffect(EffectId, EffectNode),
    /// To the compositor: create a paint slot.
    CreatePaint(PaintId, Paint),
    /// To the compositor: write a paint slot.
    SetPaint(PaintId, Paint),
    /// To the compositor: create a shader parameter slot.
    CreateParams(ParamsId, ParamsBlock),
    /// To the compositor: write a shader parameter slot.
    SetParams(ParamsId, ParamsBlock),
    /// To the compositor: create an order node.
    CreateOrder(OrderId, OrderNode),
    /// To the compositor: write an order node's effect.
    SetOrderEffect(OrderId, Option<EffectId>),
    /// To the compositor: replace entries of an order node's sequence.
    Splice {
        /// The node whose sequence changes.
        node: OrderId,
        /// The position of the first entry removed.
        at: u32,
        /// The number of entries removed.
        remove: u32,
        /// The inserted entries, a range of `TransactionParts::entries`.
        insert: ArenaRange,
    },
    /// To the compositor: set the order node an output draws.
    SetRoot(OutputId, Option<OrderId>),
    /// To the compositor: record a chunk, as an index into `TransactionParts::chunks`.
    Record(u32),
    /// To the compositor: write a chunk's default property state.
    SetProps(ChunkKey, ChunkProps),
    /// To the compositor: write a chunk's offset.
    SetOffset(ChunkKey, [f32; 2]),
    /// To the compositor: remove a chunk.
    RemoveChunk(ChunkKey),
    /// To the compositor: create an image.
    CreateImage(ImageId, ImageDesc),
    /// To the compositor: write texels of an image.
    WriteImage(ImageId, ImageWrite),
    /// To the compositor: create an external texture slot.
    CreateTexture(TextureId, TextureDesc),
    /// To the compositor: report new content bound to an external texture slot.
    TextureFrame(TextureId),
    /// To the compositor: create an application shader.
    CreateShader(ShaderId, ShaderDesc),
    /// To the compositor: start an animation.
    Animate(AnimationId, Box<Animation>),
    /// To the compositor: cancel an animation.
    CancelAnimation(AnimationId),
    /// To the compositor: remove a node, a slot or a resource.
    Remove(AnyId),
    /// To the compositor: take the declared pixels as the transaction's damage of an output.
    DeclareDamage(OutputId, Box<DamageSet>),
}
