//! Writing sampled values to a client's scene as a commit of the compositor's own.

use iceblink::{ClientId, CommitCounters};

use crate::commit::apply::Apply;
use crate::commit::{run_output, validate};
use crate::compositor::CompositorImpl;
use crate::derive::Derive;
use crate::tier::anim::Write;
use crate::tier::scroll::Change;

impl CompositorImpl {
    /// Writes animated node values and scroll changes to `client`'s scene, and derives their
    /// damage as for the same client writes. The transform nodes at the slots `rested` came to
    /// rest, and their users are measured and damaged again.
    pub(crate) fn commit_driven(
        &mut self,
        client: ClientId,
        writes: Vec<Write>,
        changes: Vec<Change>,
        rested: Vec<u32>,
    ) {
        let Some(state) = self.clients.get_mut(&client) else {
            return;
        };
        // The deferred moving damage is measured on the scene it was deferred in.
        for output in self.outputs.values_mut() {
            for pending in &mut output.scrolls {
                if pending.client == client {
                    self.work.moving_inks += pending.moving.measure(&state.scene);
                }
            }
        }

        let journal = &mut self.journal;
        journal.rested = rested;
        let mut counters = CommitCounters::default();
        let mut apply = Apply {
            scene: &mut state.scene,
            outputs: &mut self.outputs,
            journal: &mut *journal,
            counters: &mut counters,
            command: 0,
        };
        // The values are sampled from valid ones, so the writes apply.
        for write in writes {
            let _ = match write {
                Write::Transform(id, local) => apply.set_transform(id, local),
                Write::Effect(id, node) => apply.set_effect(id, node),
            };
        }
        for change in changes {
            if let Some(record) = apply
                .scene
                .scrolls
                .get_mut(change.id.slot(), change.id.generation())
            {
                record.overscroll = change.overscroll;
            }
            let _ = apply.scroll_to(change.id, change.offset);
        }

        let verdict = validate::validate(
            &state.scene,
            &self.outputs,
            journal,
            &[],
            self.fonts.as_ref(),
            &mut self.work,
            &mut self.chunk_nets,
        );

        if verdict.is_err() {
            self.chunk_nets.clear();
            journal.roll_back(&mut state.scene);
            journal.clear();
            return;
        }

        let text = run_output(&self.outputs, client);
        self.content.scale = text.scale;
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
        for output in derive.outputs.values_mut() {
            output.driving = true;
        }
        derive.run(journal, &self.chunk_nets.nets, &[]);

        journal.clear();
        self.chunk_nets.clear();
        for output in self.outputs.values_mut() {
            output.driving = false;
        }
    }
}
