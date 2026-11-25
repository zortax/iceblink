//! Turning ids into the slots items store.

use crate::id::{AnyId, ClipId, ImageId, PaintId, ParamsId, ShaderId, TextureId, TransformId};
use crate::item::{ClipRef, SpaceRef};
use crate::paint::{PaintKind, PaintRef};
use crate::transaction::recorder::ChunkRecorder;

impl ChunkRecorder<'_> {
    /// Returns the reference to the transform node `id`.
    pub fn space_ref(&mut self, id: TransformId) -> SpaceRef {
        SpaceRef(self.refer(id))
    }

    /// Returns the reference to the chain of the clip node `id`.
    pub fn clip_ref(&mut self, id: ClipId) -> ClipRef {
        ClipRef(self.refer(id))
    }

    /// Returns the reference to the paint slot `id`, whose paint has `kind`.
    pub fn paint_ref(&mut self, kind: PaintKind, id: PaintId) -> PaintRef {
        PaintRef {
            kind,
            _pad: [0; 3],
            index: self.refer(id),
        }
    }

    /// Returns the slot of the shader parameter slot `id`.
    pub fn params_slot(&mut self, id: ParamsId) -> u32 {
        self.refer(id)
    }

    /// Returns the slot of the application shader `id`.
    pub fn shader_slot(&mut self, id: ShaderId) -> u32 {
        self.refer(id)
    }

    /// Returns the slot of the image `id`.
    pub fn image_slot(&mut self, id: ImageId) -> u32 {
        self.refer(id)
    }

    /// Returns the slot of the external texture slot `id`.
    pub fn texture_slot(&mut self, id: TextureId) -> u32 {
        self.refer(id)
    }

    /// Adds `id` to the chunk's `refs` and returns its slot.
    fn refer(&mut self, id: impl Into<AnyId>) -> u32 {
        let any = id.into();

        // Consecutive references to one id need one entry.
        if self.chunk.refs.last() != Some(&any) {
            self.chunk.refs.push(any);
        }

        any.parts().0
    }
}
