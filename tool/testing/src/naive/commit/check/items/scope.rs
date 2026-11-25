//! The slots an item refers to, checked against its client and its chunk.

use iceblink::FontRegistry;
use iceblink::{
    AnyId, ClipRef, FontId, PaintKind, PaintRef, RecordedChunk, RejectReason, ShaderMode, SpaceRef,
};

use crate::naive::commit::check::nodes::valid;
use crate::naive::scene::Client;

/// A recorded chunk, with the client whose objects its slots refer to and the fonts its runs use.
pub(super) struct Scope<'a> {
    pub(super) client: &'a Client,
    pub(super) chunk: &'a RecordedChunk,
    pub(super) fonts: &'a dyn FontRegistry,
}

impl Scope<'_> {
    /// Fails with `StaleId` unless every id of the chunk's `refs` is live.
    pub(super) fn refs(&self) -> Result<(), RejectReason> {
        if self.chunk.refs.iter().all(|id| is_live(self.client, *id)) {
            Ok(())
        } else {
            Err(RejectReason::StaleId)
        }
    }

    pub(super) fn space(&self, space: SpaceRef) -> Result<(), RejectReason> {
        valid(space == SpaceRef::INHERIT || self.client.transforms.at(space.0).is_some())
    }

    pub(super) fn clip(&self, clip: ClipRef) -> Result<(), RejectReason> {
        let exists = if clip == ClipRef::INHERIT || clip == ClipRef::NONE {
            true
        } else if let Some(local) = clip.local_index() {
            (local as usize) < self.chunk.local_clips.len()
        } else {
            self.client.clips.at(clip.0).is_some()
        };
        valid(exists)
    }

    pub(super) fn paint(&self, paint: PaintRef) -> Result<(), RejectReason> {
        if !paint.kind.is_valid() {
            return Err(RejectReason::InvalidItem);
        }

        if paint.kind == PaintKind::NONE {
            return Ok(());
        }

        if paint.kind == PaintKind::INLINE {
            let [r, g, b, a] = paint.index.to_le_bytes();
            return valid(r <= a && g <= a && b <= a);
        }

        let held = self
            .client
            .paints
            .at(paint.index)
            .ok_or(RejectReason::InvalidItem)?;
        if held.kind() == paint.kind {
            Ok(())
        } else {
            Err(RejectReason::PaintKindChanged)
        }
    }

    /// Checks a shader-quad shader, which must be in paint or coverage mode.
    pub(super) fn quad_shader(&self, slot: u32) -> Result<(), RejectReason> {
        let desc = self
            .client
            .shaders
            .at(slot)
            .ok_or(RejectReason::InvalidItem)?;
        if desc.mode == ShaderMode::PAINT || desc.mode == ShaderMode::COVERAGE {
            Ok(())
        } else {
            Err(RejectReason::InvalidShader)
        }
    }

    pub(super) fn params(&self, slot: u32) -> Result<(), RejectReason> {
        valid(self.client.params.at(slot).is_some())
    }

    pub(super) fn image(&self, slot: u32) -> Result<(), RejectReason> {
        valid(self.client.images.at(slot).is_some())
    }

    pub(super) fn texture(&self, slot: u32) -> Result<(), RejectReason> {
        valid(self.client.textures.at(slot).is_some())
    }

    pub(super) fn font(&self, font: FontId) -> Result<(), RejectReason> {
        valid(self.fonts.get(font).is_some())
    }
}

/// Reports whether `id` refers to a live object of `client` with its generation.
fn is_live(client: &Client, id: AnyId) -> bool {
    match id {
        AnyId::Transform(id) => client.transforms.is_live(id),
        AnyId::Scroll(id) => client.scrolls.is_live(id),
        AnyId::Clip(id) => client.clips.is_live(id),
        AnyId::Effect(id) => client.effects.is_live(id),
        AnyId::Order(id) => client.orders.is_live(id),
        AnyId::Paint(id) => client.paints.is_live(id),
        AnyId::Params(id) => client.params.is_live(id),
        AnyId::Image(id) => client.images.is_live(id),
        AnyId::Texture(id) => client.textures.is_live(id),
        AnyId::Shader(id) => client.shaders.is_live(id),
        // The compositor holds no animations.
        AnyId::Animation(_) => false,
    }
}
