//! Removing nodes, slots and resources.

use iceblink::{AnyId, ArenaRange, CommitError, RejectReason};

use crate::commit::apply::Apply;
use crate::commit::apply::journaled::take;
use crate::commit::journal::{Change, Op};

impl Apply<'_> {
    pub(crate) fn remove(&mut self, id: AnyId) -> Result<(), CommitError> {
        let command = self.command;
        let journal = &mut *self.journal;
        let scene = &mut *self.scene;

        let removed = match id {
            AnyId::Transform(id) => take(
                command,
                &mut scene.transforms,
                &mut journal.transforms,
                id.slot(),
                id.generation(),
            ),
            AnyId::Scroll(id) => take(
                command,
                &mut scene.scrolls,
                &mut journal.scrolls,
                id.slot(),
                id.generation(),
            ),
            AnyId::Clip(id) => take(
                command,
                &mut scene.clips,
                &mut journal.clips,
                id.slot(),
                id.generation(),
            ),
            AnyId::Effect(id) => take(
                command,
                &mut scene.effects,
                &mut journal.effects,
                id.slot(),
                id.generation(),
            ),
            AnyId::Paint(id) => take(
                command,
                &mut scene.paints,
                &mut journal.paints,
                id.slot(),
                id.generation(),
            ),
            AnyId::Params(id) => take(
                command,
                &mut scene.params,
                &mut journal.params,
                id.slot(),
                id.generation(),
            ),
            AnyId::Image(id) => take(
                command,
                &mut scene.images,
                &mut journal.images,
                id.slot(),
                id.generation(),
            ),
            AnyId::Texture(id) => take(
                command,
                &mut scene.textures,
                &mut journal.textures,
                id.slot(),
                id.generation(),
            ),
            AnyId::Shader(id) => take(
                command,
                &mut scene.shaders,
                &mut journal.shaders,
                id.slot(),
                id.generation(),
            ),
            AnyId::Order(id) => return self.remove_order(id.slot(), id.generation()),
            // The tier cancels the animation once the commit succeeds, and an animation that
            // already ended cancels nothing.
            AnyId::Animation(_) => return Ok(()),
        };

        self.require(removed, RejectReason::StaleId)
    }

    /// Empties an order node's sequence and removes the node.
    fn remove_order(&mut self, slot: u32, generation: u32) -> Result<(), CommitError> {
        let Some(record) = self.scene.orders.get(slot, generation) else {
            return Err(self.reject(RejectReason::StaleId));
        };

        let len = record.entries.len() as u32;
        if len > 0 {
            self.splice_unchecked(slot, 0, len, &[], ArenaRange::default());
        }

        if let Some(record) = self.scene.orders.remove(slot) {
            let effect = record.node.effect.is_some();
            let index = self.journal.keep_order(record);
            self.journal.order_node(
                Change {
                    slot,
                    command: self.command,
                    op: Op::Removed(index),
                },
                effect,
            );
        }
        Ok(())
    }
}
