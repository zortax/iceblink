//! A driver that runs on the client's own thread.

use std::sync::Arc;

use iceblink::smallvec::SmallVec;
use iceblink::{
    ClientEvent, ClientId, CommitId, Compositor, CompositorBudgets, CompositorCapabilities,
    CompositorClient, CompositorMemory, DriverPacing, DriverStats, EventWaker, FontRegistry,
    FrameClock, FrameObserver, HitResult, IdSpace, OutputConfig, OutputId, OutputStatus,
    OutputWatch, RendererFactory, RendererTask, ScrollGesture, ScrollId, ScrollRouting,
    ScrollWatch, SubpixelOrder, Transaction, now_ns,
};

use crate::core::Driver;
use crate::mailbox::Mailbox;
use crate::options::{DriverOptions, Pacing};

/// A compositor and its outputs' renderers, driven on the client's thread.
///
/// Calls apply immediately. Frames are built when the client calls
/// [`pump`](CompositorClient::pump), for a paced driver, or [`step`](CompositorClient::step),
/// for a stepped one. Wakers are called on the calling thread before the call that queued the
/// waking event returns.
pub struct InProcess {
    driver: Driver,
    mailbox: Mailbox,
    next_commit: u64,
    /// The wakers a call owes, kept for the allocation.
    wake: Vec<EventWaker>,
}

impl InProcess {
    /// Returns a driver of `compositor`, paced as `options` say.
    pub fn new(compositor: Box<dyn Compositor>, options: DriverOptions) -> Self {
        Self {
            driver: Driver::new(compositor, options),
            mailbox: Mailbox::default(),
            next_commit: 0,
            wake: Vec::new(),
        }
    }

    /// Delivers the driver's letters and calls the wakers they owe.
    fn deliver(&mut self) {
        self.mailbox.deliver(self.driver.letters(), &mut self.wake);
        for waker in self.wake.drain(..) {
            waker();
        }
    }
}

impl CompositorClient for InProcess {
    fn capabilities(&self) -> CompositorCapabilities {
        CompositorCapabilities {
            threaded: false,
            ..self.driver.capabilities()
        }
    }

    fn pacing(&self) -> DriverPacing {
        match self.driver.options().pacing {
            Pacing::Paced => DriverPacing::Pumped,
            Pacing::Stepped => DriverPacing::Stepped,
        }
    }

    fn font_registry(&self) -> Arc<dyn FontRegistry> {
        self.driver.font_registry()
    }

    fn register_client(&mut self) -> (ClientId, IdSpace) {
        let (client, space) = self.driver.register_client();
        self.mailbox.open(client);
        (client, space)
    }

    fn unregister_client(&mut self, client: ClientId) {
        self.mailbox.close(client);
        self.driver.unregister_client(client);
        self.deliver();
    }

    fn transaction(&mut self, client: ClientId) -> Transaction {
        assert!(
            self.mailbox.contains(client),
            "the client was registered through this driver"
        );
        self.driver.transaction(client)
    }

    fn commit(&mut self, transaction: Transaction) -> CommitId {
        let commit = CommitId(self.next_commit);
        self.next_commit += 1;
        self.driver.commit(commit, transaction);
        self.deliver();
        commit
    }

    fn create_output(&mut self, config: OutputConfig, renderer: RendererFactory) -> OutputId {
        let output = self.driver.create_output(config, renderer());
        self.deliver();
        output
    }

    fn configure_output(&mut self, output: OutputId, config: OutputConfig) {
        self.driver.configure_output(output, config);
    }

    fn set_subpixel_order(&mut self, output: OutputId, order: Option<SubpixelOrder>) {
        self.driver.set_subpixel_order(output, order);
    }

    fn set_refresh(&mut self, output: OutputId, refresh_ns: Option<u64>) {
        self.driver.set_refresh(output, refresh_ns);
    }

    fn report_presented(&mut self, output: OutputId, presented_ns: u64) {
        self.driver.report_presented(output, presented_ns);
    }

    fn destroy_output(&mut self, output: OutputId) {
        self.driver.destroy_output(output);
    }

    fn request_frame(&mut self, output: OutputId) {
        self.driver.request_frame(output);
    }

    fn scroll_input(
        &mut self,
        client: ClientId,
        output: OutputId,
        mut gesture: ScrollGesture,
        predicted: ScrollRouting,
    ) {
        if self.driver.options().pacing == Pacing::Paced {
            gesture.time_ns = now_ns();
        }
        self.driver.scroll_input(client, output, gesture, predicted);
        self.deliver();
    }

    fn hit_test(&mut self, output: OutputId, point: [f32; 2]) -> SmallVec<[HitResult; 4]> {
        self.driver.hit_test(output, point)
    }

    fn watch_scroll(&mut self, client: ClientId, node: ScrollId, watch: Option<ScrollWatch>) {
        self.driver.watch_scroll(client, node, watch);
    }

    fn watch_animations(&mut self, client: ClientId, on: bool) {
        self.driver.watch_animations(client, on);
    }

    fn watch_frames(&mut self, client: ClientId, output: OutputId, on: bool) {
        self.driver.watch_frames(client, output, on);
    }

    fn watch_output(&mut self, client: ClientId, output: OutputId, watch: OutputWatch) {
        self.driver.watch_output(client, output, watch);
    }

    fn observe_frames(&mut self, output: OutputId, observer: Option<FrameObserver>) {
        self.driver.observe_frames(output, observer);
    }

    fn with_renderer(&mut self, output: OutputId, task: RendererTask) {
        self.driver.with_renderer(output, task);
    }

    fn release_cached_targets(&mut self, output: OutputId) {
        self.driver.release_cached_targets(output);
    }

    fn release_idle_resources(&mut self, output: OutputId) {
        self.driver.release_idle_resources(output);
    }

    fn set_waker(&mut self, client: ClientId, waker: Option<EventWaker>) {
        self.mailbox.set_waker(client, waker);
    }

    fn drain_events(&mut self, client: ClientId, out: &mut Vec<ClientEvent>) {
        self.mailbox.drain(client, out);
    }

    fn stats(&self) -> DriverStats {
        self.driver.stats()
    }

    fn memory(&self) -> CompositorMemory {
        self.driver.memory()
    }

    fn output_status(&self, output: OutputId) -> Option<OutputStatus> {
        self.driver.output_status(output)
    }

    fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        self.driver.extra_stats(out);
    }

    fn output_extra_stats(&self, output: OutputId, out: &mut dyn FnMut(&'static str, u64)) {
        self.driver.output_extra_stats(output, out);
    }

    fn trim(&mut self, to_bytes: u64) {
        self.driver.trim(to_bytes);
    }

    fn forget_caches(&mut self) {
        self.driver.forget_caches();
    }

    fn set_verification(&mut self, on: bool) {
        self.driver.set_verification(on);
    }

    fn set_budgets(&mut self, budgets: CompositorBudgets) {
        self.driver.set_budgets(budgets);
    }

    fn step(&mut self, clock: FrameClock) {
        self.driver.step(clock);
        self.deliver();
    }

    fn pump(&mut self) {
        self.driver.run_due(now_ns());
        self.deliver();
    }

    fn sync(&mut self) {}
}
