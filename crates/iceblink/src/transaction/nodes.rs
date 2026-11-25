//! Transaction commands on transform, scroll, clip and effect nodes.

use crate::geometry::Matrix4;
use crate::id::{ClipId, EffectId, ScrollId, TransformId};
use crate::node::{ClipNode, EffectNode, ScrollNode, TransformNode};
use crate::transaction::{Command, Transaction};

impl Transaction {
    /// Creates a transform node.
    pub fn create_transform(&mut self, node: TransformNode) -> TransformId {
        let id = self.mint();
        self.commands.push(Command::CreateTransform(id, node));
        id
    }

    /// Writes the local matrix of a transform node, ending its transform animations.
    pub fn set_transform(&mut self, id: TransformId, local: Matrix4) {
        self.commands.push(Command::SetTransform(id, local));
    }

    /// Writes a whole transform node, ending its transform animations.
    pub fn set_transform_node(&mut self, id: TransformId, node: TransformNode) {
        self.commands.push(Command::SetTransformNode(id, node));
    }

    /// Creates a scroll node.
    pub fn create_scroll(&mut self, node: ScrollNode) -> ScrollId {
        let id = self.mint();
        self.commands.push(Command::CreateScroll(id, node));
        id
    }

    /// Writes a whole scroll node, its offset as [`scroll_to`](Self::scroll_to) writes it.
    pub fn set_scroll_node(&mut self, id: ScrollId, node: ScrollNode) {
        self.commands.push(Command::SetScrollNode(id, node));
    }

    /// Writes the offset of a scroll node.
    ///
    /// The offset becomes the node's base offset, and any compositor motion and offset animation
    /// of the node ends.
    pub fn scroll_to(&mut self, id: ScrollId, offset: [f32; 2]) {
        self.commands.push(Command::ScrollTo(id, offset));
    }

    /// Adopts the compositor's scrolling of a node up to `serial`, with `offset` as the new base.
    ///
    /// Motion that follows the serial continues undisturbed.
    pub fn acknowledge_scroll(&mut self, id: ScrollId, serial: u64, offset: [f32; 2]) {
        self.commands
            .push(Command::AcknowledgeScroll { id, serial, offset });
    }

    /// Creates a clip node.
    pub fn create_clip(&mut self, node: ClipNode) -> ClipId {
        let id = self.mint();
        self.commands.push(Command::CreateClip(id, node));
        id
    }

    /// Writes a clip node.
    pub fn set_clip(&mut self, id: ClipId, node: ClipNode) {
        self.commands.push(Command::SetClip(id, node));
    }

    /// Creates an effect node.
    pub fn create_effect(&mut self, node: EffectNode) -> EffectId {
        let id = self.mint();
        self.commands.push(Command::CreateEffect(id, node));
        id
    }

    /// Writes an effect node, ending its opacity and filter animations.
    pub fn set_effect(&mut self, id: EffectId, node: EffectNode) {
        self.commands.push(Command::SetEffect(id, node));
    }
}
