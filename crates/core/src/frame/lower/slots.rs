//! Turning the client slots inside items into frame slots.

use iceblink::{ClipRef, FRAME_NO_CLIP, Matrix4, PaintKind, PaintRef, RecordedChunk, SpaceRef};

use crate::scene::Scene;
use crate::scene::record::{
    ClipRecord, NO_FRAME_SLOT, PaintRecord, ParamsRecord, ShaderRecord, TransformRecord,
};
use crate::scene::slab::Slab;
use crate::tables::FrameTables;

/// The objects whose frame slots items refer to.
#[derive(Clone, Copy)]
pub(crate) struct View<'a> {
    pub(crate) transforms: &'a Slab<TransformRecord>,
    pub(crate) clips: &'a Slab<ClipRecord>,
    pub(crate) paints: &'a Slab<PaintRecord>,
    pub(crate) params: &'a Slab<ParamsRecord>,
    pub(crate) shaders: &'a Slab<ShaderRecord>,
}

impl<'a> View<'a> {
    pub(crate) fn of(scene: &'a Scene) -> Self {
        Self {
            transforms: &scene.transforms,
            clips: &scene.clips,
            paints: &scene.paints,
            params: &scene.params,
            shaders: &scene.shaders,
        }
    }
}

/// What turning one chunk's slots into frame slots reads.
pub(crate) struct Slots<'a> {
    pub(crate) scene: View<'a>,
    pub(crate) chunk: &'a RecordedChunk,
    /// The frame clip slots of the chunk's local clips.
    pub(crate) locals: &'a [u32],
    /// The frame clip slot every item is drawn through besides its own chain, or
    /// `FRAME_NO_CLIP`.
    pub(crate) context: u32,
}

impl Slots<'_> {
    /// Returns the client slot of an item's space.
    pub(crate) fn space_slot(&self, space: SpaceRef) -> u32 {
        if space == SpaceRef::INHERIT {
            self.chunk.props.space.slot()
        } else {
            space.0
        }
    }

    /// Returns the client slot and the world matrix of an item's space.
    pub(crate) fn world(&self, space: SpaceRef) -> (u32, Matrix4) {
        let slot = self.space_slot(space);
        let world = self
            .scene
            .transforms
            .at(slot)
            .map_or(Matrix4::IDENTITY, |record| record.world);
        (slot, world)
    }

    /// Reports whether the transform node at client slot `slot` is moving.
    pub(crate) fn moving(&self, slot: u32) -> bool {
        self.scene
            .transforms
            .at(slot)
            .is_some_and(|record| record.moving)
    }

    /// Returns the frame space slot of an item's space.
    pub(crate) fn space(&self, space: SpaceRef) -> SpaceRef {
        let slot = self.space_slot(space);
        SpaceRef(
            self.scene
                .transforms
                .at(slot)
                .map_or(NO_FRAME_SLOT, |record| record.links.frame),
        )
    }

    /// Returns the frame clip slot of an item's clip, with the context outside it.
    pub(crate) fn clip(&self, tables: &mut FrameTables, clip: ClipRef) -> ClipRef {
        let own = self.own_clip(clip);
        ClipRef(tables.combine(self.context, own))
    }

    fn own_clip(&self, clip: ClipRef) -> u32 {
        if clip == ClipRef::NONE {
            return FRAME_NO_CLIP;
        }
        if clip == ClipRef::INHERIT {
            return self
                .chunk
                .props
                .clip
                .and_then(|clip| self.scene.clips.at(clip.slot()))
                .map_or(FRAME_NO_CLIP, |record| record.links.frame);
        }
        if let Some(index) = clip.local_index() {
            return self
                .locals
                .get(index as usize)
                .copied()
                .unwrap_or(FRAME_NO_CLIP);
        }
        self.scene
            .clips
            .at(clip.0)
            .map_or(FRAME_NO_CLIP, |record| record.links.frame)
    }

    /// Returns a paint reference with the frame paint slot in place of the client slot.
    ///
    /// An inline colour becomes a solid reference to the frame paint slot of the colour.
    pub(crate) fn paint(&self, tables: &FrameTables, paint: PaintRef) -> PaintRef {
        if paint.kind == PaintKind::NONE {
            return PaintRef::NONE;
        }
        if let Some(rgba) = paint.inline_color() {
            return PaintRef {
                kind: PaintKind::SOLID,
                index: tables.inline.slot(rgba).unwrap_or(NO_FRAME_SLOT),
                ..paint
            };
        }
        let index = self
            .scene
            .paints
            .at(paint.index)
            .map_or(NO_FRAME_SLOT, |record| record.links.frame);
        PaintRef { index, ..paint }
    }

    /// Returns the frame shader slot of a shader.
    pub(crate) fn shader(&self, slot: u32) -> u32 {
        self.scene
            .shaders
            .at(slot)
            .map_or(NO_FRAME_SLOT, |record| record.links.frame)
    }

    /// Returns the frame parameter slot of a parameter slot.
    pub(crate) fn params(&self, slot: u32) -> u32 {
        self.scene
            .params
            .at(slot)
            .map_or(NO_FRAME_SLOT, |record| record.links.frame)
    }
}
