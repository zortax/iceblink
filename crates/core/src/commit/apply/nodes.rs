//! Creating and writing transform, scroll, clip and effect nodes.

use iceblink::{
    ClipId, ClipNode, CommitError, EffectId, EffectNode, Matrix4, RejectReason, ScrollId,
    ScrollNode, TransformId, TransformNode,
};

use crate::commit::apply::Apply;
use crate::commit::apply::journaled::{create, write};
use crate::commit::finite;
use crate::scene::record::{ClipRecord, EffectRecord, Links, ScrollRecord, TransformRecord};

impl Apply<'_> {
    pub(crate) fn create_transform(
        &mut self,
        id: TransformId,
        node: TransformNode,
    ) -> Result<(), CommitError> {
        self.check_creatable(&self.scene.transforms, id.slot(), id.generation())?;
        self.check_transform_node(&node)?;
        self.require(finite::transform(&node), RejectReason::InvalidItem)?;

        let record = TransformRecord::new(node);
        create(
            self.command,
            &mut self.scene.transforms,
            &mut self.journal.transforms,
            id.slot(),
            id.generation(),
            record,
        );
        Ok(())
    }

    pub(crate) fn set_transform(
        &mut self,
        id: TransformId,
        local: Matrix4,
    ) -> Result<(), CommitError> {
        let Some(record) = self.scene.transforms.get(id.slot(), id.generation()) else {
            return Err(self.reject(RejectReason::StaleId));
        };
        self.require(finite::matrix(&local), RejectReason::InvalidItem)?;

        let node = TransformNode {
            local,
            ..record.node
        };
        self.write_transform(id, node)
    }

    pub(crate) fn set_transform_node(
        &mut self,
        id: TransformId,
        node: TransformNode,
    ) -> Result<(), CommitError> {
        self.require(self.live_transform(id), RejectReason::StaleId)?;
        self.check_transform_node(&node)?;
        self.require(finite::transform(&node), RejectReason::InvalidItem)?;
        self.write_transform(id, node)
    }

    fn write_transform(&mut self, id: TransformId, node: TransformNode) -> Result<(), CommitError> {
        let written = write(
            self.command,
            &mut self.scene.transforms,
            &mut self.journal.transforms,
            id.slot(),
            id.generation(),
            node,
        );
        self.require(written, RejectReason::StaleId)?;
        self.counters.property_writes += 1;
        Ok(())
    }

    pub(crate) fn create_scroll(
        &mut self,
        id: ScrollId,
        node: ScrollNode,
    ) -> Result<(), CommitError> {
        self.check_creatable(&self.scene.scrolls, id.slot(), id.generation())?;
        self.check_scroll_node(&node)?;
        self.require(finite::scroll(&node), RejectReason::InvalidItem)?;

        let record = ScrollRecord::new(clamped(node));
        create(
            self.command,
            &mut self.scene.scrolls,
            &mut self.journal.scrolls,
            id.slot(),
            id.generation(),
            record,
        );
        Ok(())
    }

    pub(crate) fn set_scroll_node(
        &mut self,
        id: ScrollId,
        node: ScrollNode,
    ) -> Result<(), CommitError> {
        self.require(self.live_scroll(id), RejectReason::StaleId)?;
        self.check_scroll_node(&node)?;
        self.require(finite::scroll(&node), RejectReason::InvalidItem)?;
        self.write_scroll(id, clamped(node))
    }

    pub(crate) fn scroll_to(&mut self, id: ScrollId, offset: [f32; 2]) -> Result<(), CommitError> {
        let Some(record) = self.scene.scrolls.get(id.slot(), id.generation()) else {
            return Err(self.reject(RejectReason::StaleId));
        };
        self.require(finite::all(&offset), RejectReason::InvalidItem)?;

        let node = clamped(ScrollNode {
            offset,
            ..record.node.clone()
        });
        self.write_scroll(id, node)
    }

    fn write_scroll(&mut self, id: ScrollId, node: ScrollNode) -> Result<(), CommitError> {
        let written = write(
            self.command,
            &mut self.scene.scrolls,
            &mut self.journal.scrolls,
            id.slot(),
            id.generation(),
            node,
        );
        self.require(written, RejectReason::StaleId)?;
        self.counters.property_writes += 1;
        Ok(())
    }

    pub(crate) fn create_clip(&mut self, id: ClipId, node: ClipNode) -> Result<(), CommitError> {
        self.check_creatable(&self.scene.clips, id.slot(), id.generation())?;
        self.check_clip_node(&node)?;
        self.require(finite::clip_shape(&node.shape), RejectReason::InvalidItem)?;

        let record = ClipRecord::new(node);
        create(
            self.command,
            &mut self.scene.clips,
            &mut self.journal.clips,
            id.slot(),
            id.generation(),
            record,
        );
        Ok(())
    }

    pub(crate) fn set_clip(&mut self, id: ClipId, node: ClipNode) -> Result<(), CommitError> {
        self.require(self.live_clip(id), RejectReason::StaleId)?;
        self.check_clip_node(&node)?;
        self.require(finite::clip_shape(&node.shape), RejectReason::InvalidItem)?;

        let written = write(
            self.command,
            &mut self.scene.clips,
            &mut self.journal.clips,
            id.slot(),
            id.generation(),
            node,
        );
        self.require(written, RejectReason::StaleId)?;
        self.counters.property_writes += 1;
        Ok(())
    }

    pub(crate) fn create_effect(
        &mut self,
        id: EffectId,
        node: EffectNode,
    ) -> Result<(), CommitError> {
        self.check_creatable(&self.scene.effects, id.slot(), id.generation())?;
        self.check_effect_node(&node)?;
        self.require(finite::effect(&node), RejectReason::InvalidItem)?;

        let record = EffectRecord {
            node,
            links: Links::default(),
            orders: Vec::new(),
        };
        create(
            self.command,
            &mut self.scene.effects,
            &mut self.journal.effects,
            id.slot(),
            id.generation(),
            record,
        );
        Ok(())
    }

    pub(crate) fn set_effect(&mut self, id: EffectId, node: EffectNode) -> Result<(), CommitError> {
        self.require(self.live_effect(id), RejectReason::StaleId)?;
        self.check_effect_node(&node)?;
        self.require(finite::effect(&node), RejectReason::InvalidItem)?;

        let written = write(
            self.command,
            &mut self.scene.effects,
            &mut self.journal.effects,
            id.slot(),
            id.generation(),
            node,
        );
        self.require(written, RejectReason::StaleId)?;
        self.counters.property_writes += 1;
        Ok(())
    }
}

/// Clamps a scroll node's offset to its range.
fn clamped(mut node: ScrollNode) -> ScrollNode {
    for axis in 0..2 {
        let limit = (node.content[axis] - node.port[axis + 2]).max(0.0);
        node.offset[axis] = node.offset[axis].max(0.0).min(limit);
    }
    node
}
