//! Applying a transaction's commands, in order, to primary state.

mod chunks;
mod declared;
mod journaled;
mod liveness;
mod nodes;
mod order;
mod paints;
mod remove;
mod resources;

use iceblink::{
    Command, CommitCounters, CommitError, RecordedChunk, RejectReason, TransactionParts,
};

use crate::commit::journal::Journal;
use crate::output::Outputs;
use crate::scene::Scene;

/// What applying one transaction works on.
pub(crate) struct Apply<'a> {
    pub(crate) scene: &'a mut Scene,
    pub(crate) outputs: &'a mut Outputs,
    pub(crate) journal: &'a mut Journal,
    pub(crate) counters: &'a mut CommitCounters,
    /// The index of the command being applied.
    pub(crate) command: u32,
}

impl Apply<'_> {
    /// Applies every command of `parts` in order, taking recorded chunks out of `parts.chunks`.
    ///
    /// # Errors
    ///
    /// Returns the first rule a command broke. The journal then holds every change made so far.
    pub(crate) fn run(&mut self, parts: &mut TransactionParts) -> Result<(), CommitError> {
        let commands = core::mem::take(&mut parts.commands);
        let result = commands
            .iter()
            .enumerate()
            .try_for_each(|(index, command)| {
                self.command = index as u32;
                if crate::tier::settles(command) {
                    self.journal.tier.commands.push(self.command);
                }
                self.one(command, parts)
            });
        parts.commands = commands;

        self.scene.placements.unplace_all(&self.journal.unplaced);
        self.journal.unplaced.clear();
        result
    }

    /// Applies `command`, the command at index `self.command`.
    fn one(&mut self, command: &Command, parts: &mut TransactionParts) -> Result<(), CommitError> {
        match command {
            Command::CreateTransform(id, node) => self.create_transform(*id, *node),
            Command::SetTransform(id, local) => self.set_transform(*id, *local),
            Command::SetTransformNode(id, node) => self.set_transform_node(*id, *node),
            Command::CreateScroll(id, node) => self.create_scroll(*id, node.clone()),
            Command::SetScrollNode(id, node) => self.scroll_node(*id, node.clone()),
            Command::ScrollTo(id, offset) => self.scroll_to_base(*id, *offset),
            Command::AcknowledgeScroll { id, serial, offset } => {
                self.acknowledge_scroll(*id, *serial, *offset)
            }
            Command::CreateClip(id, node) => self.create_clip(*id, node.clone()),
            Command::SetClip(id, node) => self.set_clip(*id, node.clone()),
            Command::CreateEffect(id, node) => self.create_effect(*id, node.clone()),
            Command::SetEffect(id, node) => self.set_effect(*id, node.clone()),
            Command::CreatePaint(id, paint) => self.create_paint(*id, paint.clone()),
            Command::SetPaint(id, paint) => self.set_paint(*id, paint.clone()),
            Command::CreateParams(id, block) => self.create_params(*id, *block),
            Command::SetParams(id, block) => self.set_params(*id, *block),
            Command::CreateOrder(id, node) => self.create_order(*id, *node),
            Command::SetOrderEffect(id, effect) => self.set_order_effect(*id, *effect),
            Command::Splice {
                node,
                at,
                remove,
                insert,
            } => self.splice(
                *node,
                *at,
                *remove,
                &parts.entries[insert.indices()],
                *insert,
            ),
            Command::SetRoot(output, root) => self.set_root(*output, *root),
            Command::Record(index) => {
                let chunk = take_chunk(&mut parts.chunks[*index as usize]);
                self.record(chunk)
            }
            Command::SetProps(key, props) => self.set_props(*key, *props),
            Command::SetOffset(key, offset) => self.set_offset(*key, *offset),
            Command::RemoveChunk(key) => self.remove_chunk(*key),
            Command::CreateImage(id, desc) => self.create_image(*id, *desc),
            Command::WriteImage(id, write) => self.write_image(*id, write.clone()),
            Command::CreateTexture(id, desc) => self.create_texture(*id, *desc),
            Command::TextureFrame(id) => self.texture_frame(*id),
            Command::CreateShader(id, desc) => self.create_shader(*id, desc.clone()),
            Command::Animate(id, animation) => self.animate(*id, animation),
            // An animation that already ended cancels nothing.
            Command::CancelAnimation(_) => Ok(()),
            Command::Remove(id) => {
                self.remove(*id)?;
                self.journal.freed.push(*id);
                Ok(())
            }
            Command::DeclareDamage(output, damage) => self.declare_damage(*output, damage),
        }
    }

    /// Returns the error for `reason`, at the command being applied.
    pub(crate) fn reject(&self, reason: RejectReason) -> CommitError {
        CommitError {
            reason,
            command: Some(self.command),
        }
    }

    /// Returns `Ok` when `holds`, and the error for `reason` otherwise.
    pub(crate) fn require(&self, holds: bool, reason: RejectReason) -> Result<(), CommitError> {
        if holds {
            Ok(())
        } else {
            Err(self.reject(reason))
        }
    }
}

/// Takes a recorded chunk out of the transaction's list, leaving an empty one with no buffers.
fn take_chunk(slot: &mut RecordedChunk) -> RecordedChunk {
    let empty = RecordedChunk {
        key: slot.key,
        props: slot.props,
        ops: Vec::new(),
        quads: Vec::new(),
        shader_quads: Vec::new(),
        shadows: Vec::new(),
        lines: Vec::new(),
        glyph_runs: Vec::new(),
        images: Vec::new(),
        marks: Vec::new(),
        paths: Vec::new(),
        externals: Vec::new(),
        backdrops: Vec::new(),
        glyphs: Vec::new(),
        coords: Vec::new(),
        payloads: Vec::new(),
        filters: Vec::new(),
        local_clips: Vec::new(),
        hint: None,
        refs: Vec::new(),
    };
    core::mem::replace(slot, empty)
}
