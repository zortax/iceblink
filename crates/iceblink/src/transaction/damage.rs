//! Transaction commands that declare damage.

use crate::damage::DamageSet;
use crate::id::OutputId;
use crate::transaction::{Command, Transaction};

impl Transaction {
    /// Declares that the transaction changes no pixel of `output` outside `damage`, measured on
    /// the output's device as it is configured at commit.
    ///
    /// A compositor may then take `damage` as what the transaction adds to the output's damage,
    /// in place of what it would derive from the changes. Scroll offset changes, which a frame may
    /// answer with a shift, keep their derived damage. A later declaration for the same output
    /// adds to an earlier one.
    pub fn declare_damage(&mut self, output: OutputId, damage: DamageSet) {
        self.commands
            .push(Command::DeclareDamage(output, Box::new(damage)));
    }
}
