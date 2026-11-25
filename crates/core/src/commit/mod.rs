//! Committing a transaction: apply, validate, then derive or roll back.

pub(crate) mod apply;
mod declared;
pub(crate) mod finite;
pub(crate) mod journal;
pub(crate) mod net;
pub(crate) mod precedence;
pub(crate) mod validate;

use iceblink::CommitReport;
use iceblink::{
    ClientId, CommitCounters, CommitError, RejectReason, Transaction, TransactionParts,
};

use crate::compositor::CompositorImpl;
use crate::derive::Derive;
use crate::output::Outputs;
use crate::scene::Scene;
use crate::shared_content::prefetch::RunOutput;

use self::apply::Apply;
use self::journal::Journal;

impl CompositorImpl {
    /// Applies a transaction atomically, then derives the damage and frame state of its changes.
    ///
    /// # Errors
    ///
    /// Returns `StaleId` for a transaction of an unknown client or of an outdated id space, and
    /// otherwise the rule the transaction broke. Every change is then rolled back and the ids the
    /// transaction minted are freed.
    pub(crate) fn commit_transaction(
        &mut self,
        transaction: Transaction,
    ) -> Result<CommitReport, CommitError> {
        let client = transaction.client();
        let mut parts = transaction.into_parts();

        let Some(state) = self.clients.get_mut(&client) else {
            return Err(refuse(parts, RejectReason::StaleId));
        };
        if !state.ids.same(&parts.ids) {
            return Err(refuse(parts, RejectReason::StaleId));
        }

        // The deferred moving damage is measured on the scene it was deferred in.
        for output in self.outputs.values_mut() {
            for pending in &mut output.scrolls {
                if pending.client == client {
                    self.work.moving_inks += pending.moving.measure(&state.scene);
                }
            }
        }

        let journal = &mut self.journal;
        let mut counters = CommitCounters::default();
        let mut apply = Apply {
            scene: &mut state.scene,
            outputs: &mut self.outputs,
            journal: &mut *journal,
            counters: &mut counters,
            command: 0,
        };
        let verdict = apply.run(&mut parts).and_then(|()| {
            validate::validate(
                &state.scene,
                &self.outputs,
                journal,
                &parts.entries,
                self.fonts.as_ref(),
                &mut self.work,
                &mut self.chunk_nets,
            )
        });

        if let Err(error) = verdict {
            self.chunk_nets.clear();
            roll_back(journal, &mut state.scene, &mut self.outputs);
            parts.ids.free(&parts.minted);
            journal.clear();
            return Err(error);
        }

        let mut declared = core::mem::take(&mut journal.declared);
        let stage = core::mem::take(&mut journal.tier);
        if !journal.scrolls.is_empty() {
            declared.clear();
        }
        declared::divert(&mut self.outputs, &declared, self.verification);

        let text = run_output(&self.outputs, client);
        self.content.scale = text.scale;
        let before = self.content.rasterised();
        let mut derive = Derive {
            scene: &mut state.scene,
            outputs: &mut self.outputs,
            tables: &mut self.tables,
            revisions: &mut self.revisions,
            content: &mut self.content,
            spare: &mut state.recycled.returned,
            events: &mut self.events,
            counters: &mut counters,
            hint_violations: &mut self.counters.hint_violations,
            mismatches: &mut self.mismatches,
            serial: self.serial,
            verification: self.verification,
            text,
            work: &mut self.work,
        };
        derive.run(journal, &self.chunk_nets.nets, &parts.entries);
        self.chunk_nets.clear();

        declared::settle(
            &mut self.outputs,
            &declared,
            client,
            &mut self.events,
            &mut self.counters.damage_violations,
        );
        declared.clear();
        journal.declared = declared;

        let mut freed = core::mem::take(&mut journal.freed);
        journal.clear();

        let animations = crate::tier::settle(
            &mut state.scene,
            &state.ids,
            &parts.commands,
            stage,
            &mut self.events,
        );

        state.ids.free(&freed);
        freed.clear();
        self.journal.freed = freed;

        counters.glyphs_rasterised = (self.content.rasterised() - before) as u32;
        state.keep(parts);

        self.counters.commit = counters;
        Ok(CommitReport {
            animations,
            counters,
        })
    }
}

/// Returns what the output that draws `client` allows its glyph runs.
pub(crate) fn run_output(outputs: &Outputs, client: ClientId) -> RunOutput {
    let output = outputs
        .values()
        .find(|output| output.root.is_some_and(|(owner, _)| owner == client));

    RunOutput {
        subpixel: output.and_then(|output| {
            let (caps, order) = output.last_request?;
            order.filter(|_| caps.subpixel_text && output.config.opaque)
        }),
        scale: output.map_or(1.0, |output| output.config.scale),
    }
}

/// Undoes every change in `journal` to `scene` and to the roots of `outputs`.
#[cold]
fn roll_back(journal: &mut Journal, scene: &mut Scene, outputs: &mut Outputs) {
    journal.roll_back(scene);
    for write in journal.roots.drain(..).rev() {
        if let Some(output) = outputs.get_mut(&write.output) {
            output.root = write.before;
        }
    }
}

/// Frees the ids a refused transaction minted and returns the error for `reason`.
#[cold]
fn refuse(parts: TransactionParts, reason: RejectReason) -> CommitError {
    parts.ids.free(&parts.minted);
    CommitError {
        reason,
        command: None,
    }
}
