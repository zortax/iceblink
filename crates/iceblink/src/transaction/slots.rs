//! Transaction commands on paint and shader parameter slots.

use crate::id::{PaintId, ParamsId};
use crate::paint::{Paint, ParamsBlock};
use crate::transaction::{Command, Transaction};

impl Transaction {
    /// Creates a paint slot.
    pub fn create_paint(&mut self, paint: Paint) -> PaintId {
        let id = self.mint();
        self.commands.push(Command::CreatePaint(id, paint));
        id
    }

    /// Rewrites a paint slot with a paint of the same kind.
    pub fn set_paint(&mut self, id: PaintId, paint: Paint) {
        self.commands.push(Command::SetPaint(id, paint));
    }

    /// Creates a shader parameter slot.
    pub fn create_params(&mut self, block: ParamsBlock) -> ParamsId {
        let id = self.mint();
        self.commands.push(Command::CreateParams(id, block));
        id
    }

    /// Rewrites a shader parameter slot.
    pub fn set_params(&mut self, id: ParamsId, block: ParamsBlock) {
        self.commands.push(Command::SetParams(id, block));
    }
}
