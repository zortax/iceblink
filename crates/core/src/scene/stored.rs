//! The value a transaction writes into each kind of stored object.

use iceblink::{ClipNode, EffectNode, OrderNode, Paint, ParamsBlock, ScrollNode, TransformNode};

use crate::scene::record::{
    ClipRecord, EffectRecord, ImageRecord, Links, OrderRecord, PaintRecord, ParamsRecord,
    ScrollRecord, ShaderRecord, TextureRecord, TransformRecord,
};

/// A stored object whose written value can be read, replaced and restored.
pub(crate) trait Stored {
    /// The value a write replaces.
    type Value: Clone + core::fmt::Debug + crate::scene::names::Names;

    fn value(&self) -> &Self::Value;
    fn value_mut(&mut self) -> &mut Self::Value;
    fn links(&self) -> &Links;
}

macro_rules! stored {
    ($($record:ty => $field:ident: $value:ty),+ $(,)?) => {
        $(
            impl Stored for $record {
                type Value = $value;

                fn value(&self) -> &$value {
                    &self.$field
                }

                fn value_mut(&mut self) -> &mut $value {
                    &mut self.$field
                }

                fn links(&self) -> &Links {
                    &self.links
                }
            }
        )+
    };
}

stored! {
    TransformRecord => node: TransformNode,
    ScrollRecord => node: ScrollNode,
    ClipRecord => node: ClipNode,
    EffectRecord => node: EffectNode,
    OrderRecord => node: OrderNode,
    PaintRecord => paint: Paint,
    ParamsRecord => block: ParamsBlock,
}

macro_rules! unwritable {
    ($($record:ty),+ $(,)?) => {
        $(
            impl Stored for $record {
                type Value = ();

                fn value(&self) -> &() {
                    &()
                }

                fn value_mut(&mut self) -> &mut () {
                    // Resources are created and removed but never replaced.
                    unreachable!("a resource has no written value")
                }

                fn links(&self) -> &Links {
                    &self.links
                }
            }
        )+
    };
}

unwritable!(ImageRecord, TextureRecord, ShaderRecord);
