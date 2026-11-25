//! Checks that the ids a command passes refer to live objects.

use iceblink::{
    Anchor, ChunkProps, ClipId, ClipNode, ClipShape, CommitError, EffectId, EffectNode, Filter,
    ImageId, LocalClip, LocalParent, OrderId, ParamsId, RejectReason, ScrollId, ScrollNode,
    ShaderId, ShaderMode, TextureId, TransformId, TransformNode,
};

use crate::commit::apply::Apply;

impl Apply<'_> {
    pub(crate) fn live_transform(&self, id: TransformId) -> bool {
        self.scene
            .transforms
            .get(id.slot(), id.generation())
            .is_some()
    }

    pub(crate) fn live_scroll(&self, id: ScrollId) -> bool {
        self.scene.scrolls.get(id.slot(), id.generation()).is_some()
    }

    pub(crate) fn live_clip(&self, id: ClipId) -> bool {
        self.scene.clips.get(id.slot(), id.generation()).is_some()
    }

    pub(crate) fn live_effect(&self, id: EffectId) -> bool {
        self.scene.effects.get(id.slot(), id.generation()).is_some()
    }

    pub(crate) fn live_order(&self, id: OrderId) -> bool {
        self.scene.orders.get(id.slot(), id.generation()).is_some()
    }

    pub(crate) fn live_params(&self, id: ParamsId) -> bool {
        self.scene.params.get(id.slot(), id.generation()).is_some()
    }

    pub(crate) fn live_image(&self, id: ImageId) -> bool {
        self.scene.images.get(id.slot(), id.generation()).is_some()
    }

    pub(crate) fn live_texture(&self, id: TextureId) -> bool {
        self.scene
            .textures
            .get(id.slot(), id.generation())
            .is_some()
    }

    pub(crate) fn live_shader(&self, id: ShaderId) -> bool {
        self.scene.shaders.get(id.slot(), id.generation()).is_some()
    }

    /// Checks that the ids a transform node holds are live.
    pub(crate) fn check_transform_node(&self, node: &TransformNode) -> Result<(), CommitError> {
        let parent = node.parent.is_none_or(|id| self.live_transform(id));
        let scroll = node.scroll.is_none_or(|id| self.live_scroll(id));
        let anchored = match node.anchor {
            Anchor::Sticky(sticky) => self.live_scroll(sticky.scroll),
            Anchor::Thumb(thumb) => self.live_scroll(thumb.scroll),
            Anchor::Scrolls | Anchor::Fixed => true,
        };
        self.require(parent && scroll && anchored, RejectReason::StaleId)
    }

    /// Checks that the id a scroll node holds is live.
    pub(crate) fn check_scroll_node(&self, node: &ScrollNode) -> Result<(), CommitError> {
        self.require(self.live_transform(node.space), RejectReason::StaleId)
    }

    /// Checks that the ids a clip node holds are live.
    pub(crate) fn check_clip_node(&self, node: &ClipNode) -> Result<(), CommitError> {
        let parent = node.parent.is_none_or(|id| self.live_clip(id));
        let space = self.live_transform(node.space);
        self.require(parent && space, RejectReason::StaleId)?;
        self.check_clip_shape(&node.shape)
    }

    /// Checks that the image of a mask clip shape is live.
    pub(crate) fn check_clip_shape(&self, shape: &ClipShape) -> Result<(), CommitError> {
        match shape {
            ClipShape::RoundedRect { .. } => Ok(()),
            ClipShape::Mask { image, .. } => {
                self.require(self.live_image(*image), RejectReason::StaleId)
            }
        }
    }

    /// Checks that the ids an effect node holds are live, and that its shaders are filters.
    pub(crate) fn check_effect_node(&self, node: &EffectNode) -> Result<(), CommitError> {
        let clip = node.clip.is_none_or(|id| self.live_clip(id));
        self.require(clip, RejectReason::StaleId)?;
        self.check_filter_ids(&node.filters)?;

        for filter in node.filters.iter() {
            let Filter::Shader { shader, .. } = filter else {
                continue;
            };
            let mode = self
                .scene
                .shaders
                .get(shader.slot(), shader.generation())
                .map(|record| record.desc.mode);
            self.require(
                mode == Some(ShaderMode::FILTER),
                RejectReason::InvalidShader,
            )?;
        }
        Ok(())
    }

    /// Checks that the shaders and parameter blocks a filter chain refers to are live.
    pub(crate) fn check_filter_ids(&self, filters: &[Filter]) -> Result<(), CommitError> {
        for filter in filters {
            if let Filter::Shader { shader, params, .. } = filter {
                let live = self.live_shader(*shader) && self.live_params(*params);
                self.require(live, RejectReason::StaleId)?;
            }
        }
        Ok(())
    }

    /// Checks that the ids chunk props hold are live.
    pub(crate) fn check_props(&self, props: &ChunkProps) -> Result<(), CommitError> {
        let space = self.live_transform(props.space);
        let clip = props.clip.is_none_or(|id| self.live_clip(id));
        self.require(space && clip, RejectReason::StaleId)
    }

    /// Checks that the ids a chunk-local clip holds are live.
    pub(crate) fn check_local_clip(&self, clip: &LocalClip) -> Result<(), CommitError> {
        if let LocalParent::Node(id) = clip.parent {
            self.require(self.live_clip(id), RejectReason::StaleId)?;
        }
        self.check_clip_shape(&clip.shape)
    }
}
