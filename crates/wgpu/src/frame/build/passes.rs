//! The segments of a plan: passes and their draws, encoder work, and vector round markers.

use iceblink::geom::{Device, Rect};

use crate::frame::build::PlanBuilder;
use crate::frame::segment::{EncoderOp, PassLoad, PlannedDraw, PlannedPass, Segment};
use crate::frame::target::TargetRef;

impl PlanBuilder<'_> {
    /// Moves the plan into vector round `round`, if it is not in it already.
    ///
    /// The open pass is closed before the round marker and opened again after it.
    pub(crate) fn round(&mut self, round: usize) {
        if self.round == round {
            return;
        }
        self.round = round;
        let open = self.open.as_ref().map(|pass| (pass.target, pass.scissor));
        debug_assert!(
            !matches!(open, Some((TargetRef::MarksPage(_), _))),
            "a marks page opens again with a clear, so a round must not cut one"
        );
        self.end_pass();
        self.plan.segments.push(Segment::Round(round));
        if let Some((target, scissor)) = open {
            self.begin_pass(target, scissor);
        }
    }

    /// Closes the open pass, and opens a pass over `target` cut to `scissor`.
    ///
    /// The pass discards what its target holds when the target is a lent slot no pass has written
    /// yet, or a mark page. Otherwise it keeps it.
    pub(crate) fn begin_pass(&mut self, target: TargetRef, scissor: Rect<i32, Device>) {
        self.end_pass();
        let globals = self.globals_for(target);
        let load = match target.slot() {
            Some(slot) if self.unwritten.contains(&slot) => {
                self.unwritten.retain(|held| *held != slot);
                PassLoad::Discard
            }
            // A bin page holds only this frame's coverage.
            _ if matches!(target, TargetRef::MarksPage(_)) => PassLoad::Discard,
            _ => PassLoad::Keep,
        };
        let start = self.plan.draws.len();
        self.open = Some(PlannedPass {
            target,
            load,
            scissor,
            globals,
            draws: start..start,
        });
    }

    /// Appends `draw` to the open pass.
    ///
    /// # Panics
    ///
    /// Panics when no pass is open.
    pub(crate) fn draw(&mut self, draw: PlannedDraw) {
        let open = self
            .open
            .as_mut()
            .expect("a draw is always planned into an open pass");
        self.plan.draws.push(draw);
        open.draws.end = self.plan.draws.len();
    }

    /// Closes the open pass, if there is one.
    ///
    /// A pass with no draws or an empty scissor is dropped.
    pub(crate) fn end_pass(&mut self) {
        let Some(pass) = self.open.take() else {
            return;
        };
        if pass.draws.is_empty() || pass.scissor.is_empty() {
            // A pass that is not recorded did not discard anything either, and the lease still
            // owes its first clear to whichever pass writes it next.
            if pass.load == PassLoad::Discard
                && let Some(slot) = pass.target.slot()
            {
                self.unwritten.push(slot);
            }
            return;
        }
        self.plan.segments.push(Segment::Pass(pass));
    }

    /// Closes the open pass, and appends `op`.
    pub(crate) fn encoder(&mut self, op: EncoderOp) {
        self.end_pass();
        self.plan.segments.push(Segment::Encoder(op));
    }
}
