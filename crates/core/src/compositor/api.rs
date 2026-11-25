//! The `Compositor` implementation of the reference compositor.

use std::sync::Arc;

use iceblink::smallvec::SmallVec;
use iceblink::{
    Built, ClientId, CommitError, CompositorEvent, Counters, Frame, FrameId, FrameOutcome,
    FrameRequest, HitResult, IdSpace, OutputId, ScrollGesture, ScrollRouting, Transaction,
};
use iceblink::{
    CommitReport, Compositor, CompositorBudgets, CompositorCapabilities, CompositorMemory,
    FontRegistry, GlyphRaster, OutputConfig,
};

use crate::compositor::CompositorImpl;
use crate::tier::earliest;

impl Compositor for CompositorImpl {
    fn capabilities(&self) -> CompositorCapabilities {
        CompositorCapabilities {
            threaded: false,
            animations: true,
            scroll_input: true,
            hit_test: true,
            precise: true,
        }
    }

    fn register_client(&mut self) -> (ClientId, IdSpace) {
        self.add_client()
    }

    fn unregister_client(&mut self, client: ClientId) {
        self.drop_client(client);
    }

    fn transaction(&mut self, client: ClientId) -> Transaction {
        self.client_transaction(client)
    }

    fn commit(&mut self, transaction: Transaction) -> Result<CommitReport, CommitError> {
        self.commit_transaction(transaction)
    }

    fn use_font_registry(&mut self, registry: Arc<dyn FontRegistry>) {
        self.content.set_fonts(Arc::clone(&registry));
        self.fonts = registry;
        self.restart_text();
    }

    fn font_registry(&self) -> Arc<dyn FontRegistry> {
        Arc::clone(&self.fonts)
    }

    fn use_glyph_raster(&mut self, raster: Arc<dyn GlyphRaster>) {
        self.content.set_raster(raster);
        self.restart_text();
    }

    fn create_output(&mut self, config: OutputConfig) -> OutputId {
        self.add_output(config)
    }

    fn configure_output(&mut self, output: OutputId, config: OutputConfig) {
        self.reconfigure_output(output, config);
    }

    fn destroy_output(&mut self, output: OutputId) {
        self.remove_output(output);
    }

    fn build_frame(
        &mut self,
        output: OutputId,
        request: &FrameRequest<'_>,
        into: &mut Frame,
    ) -> Built {
        self.build(output, request, into)
    }

    fn next_wake(&self, output: OutputId) -> Option<u64> {
        let wake = self.outputs.get(&output)?.wake;
        // A frame path that keeps its route history waits for frames to be counted.
        earliest(wake, self.content.cache.routes_waiting().then_some(0))
    }

    fn frame_done(&mut self, output: OutputId, frame: FrameId, outcome: &FrameOutcome) {
        self.done(output, frame, outcome);
    }

    fn scroll_input(&mut self, output: OutputId, gesture: ScrollGesture) -> ScrollRouting {
        self.scroll_gesture(output, gesture)
    }

    fn hit_test(&self, output: OutputId, point: [f32; 2]) -> SmallVec<[HitResult; 4]> {
        self.hits(output, point)
    }

    fn drain_events(&mut self, out: &mut Vec<CompositorEvent>) {
        out.append(&mut self.events);
    }

    fn set_verification(&mut self, on: bool) {
        self.verification = on;
    }

    fn counters(&self) -> &Counters {
        &self.counters
    }

    fn memory(&self) -> CompositorMemory {
        self.memory_report()
    }

    fn set_budgets(&mut self, budgets: CompositorBudgets) {
        self.set_budget_limits(budgets);
    }

    fn budgets(&self) -> CompositorBudgets {
        self.budgets
    }

    fn trim(&mut self, to_bytes: u64) -> u64 {
        self.trim_atlas(to_bytes)
    }

    fn forget_caches(&mut self) -> u64 {
        self.forget()
    }

    fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        out("verification_mismatches", self.mismatches);
        out("frame_check_failures", self.check_failures);
        out("order_overlaps", self.order_overlaps);
        out("combined_clips", self.tables.combined.len() as u64);
        self.content.cache.stats().report(out);
        out(
            "content.page_copy_bytes",
            self.content.cache.page_copy_bytes(),
        );
        self.work.report(out);
        self.refusals.report(out);
    }
}
