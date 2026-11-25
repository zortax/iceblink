//! Declared damage.

use iceblink::damage::DamageSet;
use iceblink::{CommitError, OutputId, RejectReason};

use crate::commit::apply::Apply;

impl Apply<'_> {
    /// Adds `damage` to what the transaction declares for `output`.
    pub(crate) fn declare_damage(
        &mut self,
        output: OutputId,
        damage: &DamageSet,
    ) -> Result<(), CommitError> {
        self.require(
            self.outputs.contains_key(&output),
            RejectReason::UnknownOutput,
        )?;

        let declared = &mut self.journal.declared;
        match declared.iter_mut().find(|(held, _)| *held == output) {
            Some((_, held)) => held.absorb_set(damage),
            None => declared.push((output, *damage)),
        }
        Ok(())
    }
}
