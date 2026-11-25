//! The memory snapshot of a threaded driver reflects a trim once the driver applied it.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use iceblink::smallvec::SmallVec;
use iceblink::{
    Built, ClientId, CommitError, CompositorEvent, Counters, Frame, FrameId, FrameOutcome,
    FrameRequest, HitResult, IdSpace, OutputId, ScrollGesture, ScrollRouting, Transaction,
};
use iceblink::{
    CommitReport, Compositor, CompositorBudgets, CompositorCapabilities, CompositorClient,
    CompositorMemory, FontRegistry, GlyphRaster, OutputConfig,
};
use iceblink_core::CompositorImpl;
use iceblink_driver::thread::ThreadedCompositor;

/// The reference compositor, with atlas pages of `atlas` bytes that a trim shrinks.
struct Trimmed {
    inner: CompositorImpl,
    atlas: u64,
}

impl Compositor for Trimmed {
    fn capabilities(&self) -> CompositorCapabilities {
        self.inner.capabilities()
    }

    fn register_client(&mut self) -> (ClientId, IdSpace) {
        self.inner.register_client()
    }

    fn unregister_client(&mut self, client: ClientId) {
        self.inner.unregister_client(client);
    }

    fn transaction(&mut self, client: ClientId) -> Transaction {
        self.inner.transaction(client)
    }

    fn commit(&mut self, transaction: Transaction) -> Result<CommitReport, CommitError> {
        self.inner.commit(transaction)
    }

    fn use_font_registry(&mut self, registry: Arc<dyn FontRegistry>) {
        self.inner.use_font_registry(registry);
    }

    fn font_registry(&self) -> Arc<dyn FontRegistry> {
        self.inner.font_registry()
    }

    fn use_glyph_raster(&mut self, raster: Arc<dyn GlyphRaster>) {
        self.inner.use_glyph_raster(raster);
    }

    fn create_output(&mut self, config: OutputConfig) -> OutputId {
        self.inner.create_output(config)
    }

    fn configure_output(&mut self, output: OutputId, config: OutputConfig) {
        self.inner.configure_output(output, config);
    }

    fn destroy_output(&mut self, output: OutputId) {
        self.inner.destroy_output(output);
    }

    fn build_frame(
        &mut self,
        output: OutputId,
        request: &FrameRequest<'_>,
        into: &mut Frame,
    ) -> Built {
        self.inner.build_frame(output, request, into)
    }

    fn frame_done(&mut self, output: OutputId, frame: FrameId, outcome: &FrameOutcome) {
        self.inner.frame_done(output, frame, outcome);
    }

    fn scroll_input(&mut self, output: OutputId, gesture: ScrollGesture) -> ScrollRouting {
        self.inner.scroll_input(output, gesture)
    }

    fn hit_test(&self, output: OutputId, point: [f32; 2]) -> SmallVec<[HitResult; 4]> {
        self.inner.hit_test(output, point)
    }

    fn drain_events(&mut self, out: &mut Vec<CompositorEvent>) {
        self.inner.drain_events(out);
    }

    fn set_verification(&mut self, on: bool) {
        self.inner.set_verification(on);
    }

    fn counters(&self) -> &Counters {
        self.inner.counters()
    }

    fn memory(&self) -> CompositorMemory {
        CompositorMemory {
            atlas_bytes: self.atlas,
            ..self.inner.memory()
        }
    }

    fn set_budgets(&mut self, budgets: CompositorBudgets) {
        self.inner.set_budgets(budgets);
    }

    fn budgets(&self) -> CompositorBudgets {
        self.inner.budgets()
    }

    fn trim(&mut self, to_bytes: u64) -> u64 {
        let freed = self.atlas.saturating_sub(to_bytes);
        self.atlas -= freed;
        freed
    }

    fn forget_caches(&mut self) -> u64 {
        std::mem::take(&mut self.atlas)
    }

    fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        self.inner.extra_stats(out);
    }
}

#[test]
fn a_trim_shows_in_the_memory_snapshot_once_applied() {
    let compositor = Trimmed {
        inner: CompositorImpl::new(),
        atlas: 1 << 20,
    };
    let (driver, mut handle) = ThreadedCompositor::spawn(Box::new(compositor));
    handle.sync();
    assert_eq!(handle.memory().atlas_bytes, 1 << 20);

    // No later call renews the snapshot, so only the trim's own publication can show it.
    handle.trim(0);
    let start = Instant::now();
    while handle.memory().atlas_bytes != 0 && start.elapsed() < Duration::from_secs(2) {
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        handle.memory().atlas_bytes,
        0,
        "the snapshot shows the trimmed atlas"
    );
    driver.shutdown();
}
