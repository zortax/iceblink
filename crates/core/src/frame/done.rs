//! Retiring a built frame by the outcome of drawing it.

use iceblink::{FrameId, FrameOutcome, OutputId, SkipReason};
use rustc_hash::FxHashSet;

use crate::compositor::CompositorImpl;
use crate::frame::lower::unpin_resident;
use crate::output::renderer::RendererState;

impl CompositorImpl {
    /// Retires or keeps the damage of a frame by the outcome of drawing it.
    pub(crate) fn done(&mut self, id: OutputId, frame: FrameId, outcome: &FrameOutcome) {
        let Some(output) = self.outputs.get_mut(&id) else {
            return;
        };
        let Some(flight) = output.flight.take().filter(|flight| flight.id == frame) else {
            return;
        };

        if outcome.retires_damage() {
            output.history.push(flight.present);
            output.renderer.kept = true;
        } else {
            output.pending.absorb_set(&flight.damage);
            for shift in &flight.shifts {
                output.pending.absorb(shift.region);
            }
            output.renderer.kept = false;
        }

        if matches!(
            outcome,
            FrameOutcome::Recovered | FrameOutcome::Skipped(SkipReason::DeviceUnavailable)
        ) {
            let lost = core::mem::replace(&mut output.renderer, RendererState::fresh());
            output.history.clear();
            output.damage_all();
            self.release_resident(&lost.resident);
        }
    }

    /// Releases the pin of every lowering whose revision is in `revisions` and that no output's
    /// renderer holds resident.
    pub(crate) fn release_resident(&mut self, revisions: &FxHashSet<u64>) {
        if revisions.is_empty() {
            return;
        }
        let held = |revision: u64| {
            self.outputs
                .values()
                .any(|output| output.renderer.resident.contains(&revision))
        };

        // A renderer loses what it holds rarely, so every lowering is looked at.
        for client in self.clients.values_mut() {
            for record in client.scene.chunks.values_mut() {
                let Some(lowered) = record.derived.lowered.as_mut() else {
                    continue;
                };
                if revisions.contains(&lowered.revision) && !held(lowered.revision) {
                    unpin_resident(lowered, &mut self.content);
                }
            }
        }
    }

    /// Makes reusable the frame slots and log entries every output has moved past.
    ///
    /// A fresh renderer holds nothing back, since its next frame replaces everything it holds.
    pub(crate) fn settle(&mut self) {
        let held = || {
            self.outputs
                .values()
                .filter(|output| !output.renderer.fresh)
        };

        let built = held().map(|output| output.built).min();
        self.tables.settle(built.unwrap_or(self.serial));

        let retired = held().map(|output| output.renderer.retired).min();
        self.revisions
            .settle(retired.unwrap_or(self.revisions.position()));

        let received = held().map(|output| output.renderer.pages.next).min();
        self.content
            .pages
            .settle(received.unwrap_or(self.content.pages.end()));
    }
}
