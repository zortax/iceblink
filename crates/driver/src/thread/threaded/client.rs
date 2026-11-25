//! The owner's calls of the client seam, made through its own handle.

use std::sync::Arc;

use iceblink::smallvec::SmallVec;
use iceblink::{
    ClientEvent, ClientId, CommitId, CompositorBudgets, CompositorCapabilities, CompositorClient,
    CompositorMemory, DriverPacing, DriverStats, EventWaker, FontRegistry, FrameClock,
    FrameObserver, HitResult, IdSpace, OutputConfig, OutputId, OutputStatus, OutputWatch,
    RendererFactory, RendererTask, ScrollGesture, ScrollId, ScrollRouting, ScrollWatch,
    SubpixelOrder, Transaction,
};

use crate::thread::threaded::ThreadedCompositor;

impl CompositorClient for ThreadedCompositor {
    fn capabilities(&self) -> CompositorCapabilities {
        self.handle.capabilities()
    }

    fn pacing(&self) -> DriverPacing {
        self.handle.pacing()
    }

    fn font_registry(&self) -> Arc<dyn FontRegistry> {
        self.handle.font_registry()
    }

    fn register_client(&mut self) -> (ClientId, IdSpace) {
        self.handle.register_client()
    }

    fn unregister_client(&mut self, client: ClientId) {
        self.handle.unregister_client(client);
    }

    fn transaction(&mut self, client: ClientId) -> Transaction {
        self.handle.transaction(client)
    }

    fn commit(&mut self, transaction: Transaction) -> CommitId {
        self.handle.commit(transaction)
    }

    fn create_output(&mut self, config: OutputConfig, renderer: RendererFactory) -> OutputId {
        self.handle.create_output(config, renderer)
    }

    fn configure_output(&mut self, output: OutputId, config: OutputConfig) {
        self.handle.configure_output(output, config);
    }

    fn set_subpixel_order(&mut self, output: OutputId, order: Option<SubpixelOrder>) {
        self.handle.set_subpixel_order(output, order);
    }

    fn set_refresh(&mut self, output: OutputId, refresh_ns: Option<u64>) {
        self.handle.set_refresh(output, refresh_ns);
    }

    fn report_presented(&mut self, output: OutputId, presented_ns: u64) {
        self.handle.report_presented(output, presented_ns);
    }

    fn destroy_output(&mut self, output: OutputId) {
        self.handle.destroy_output(output);
    }

    fn request_frame(&mut self, output: OutputId) {
        self.handle.request_frame(output);
    }

    fn scroll_input(
        &mut self,
        client: ClientId,
        output: OutputId,
        gesture: ScrollGesture,
        predicted: ScrollRouting,
    ) {
        self.handle.scroll_input(client, output, gesture, predicted);
    }

    fn hit_test(&mut self, output: OutputId, point: [f32; 2]) -> SmallVec<[HitResult; 4]> {
        self.handle.hit_test(output, point)
    }

    fn watch_scroll(&mut self, client: ClientId, node: ScrollId, watch: Option<ScrollWatch>) {
        self.handle.watch_scroll(client, node, watch);
    }

    fn watch_animations(&mut self, client: ClientId, on: bool) {
        self.handle.watch_animations(client, on);
    }

    fn watch_frames(&mut self, client: ClientId, output: OutputId, on: bool) {
        self.handle.watch_frames(client, output, on);
    }

    fn watch_output(&mut self, client: ClientId, output: OutputId, watch: OutputWatch) {
        self.handle.watch_output(client, output, watch);
    }

    fn observe_frames(&mut self, output: OutputId, observer: Option<FrameObserver>) {
        self.handle.observe_frames(output, observer);
    }

    fn with_renderer(&mut self, output: OutputId, task: RendererTask) {
        self.handle.with_renderer(output, task);
    }

    fn release_cached_targets(&mut self, output: OutputId) {
        self.handle.release_cached_targets(output);
    }

    fn release_idle_resources(&mut self, output: OutputId) {
        self.handle.release_idle_resources(output);
    }

    fn set_waker(&mut self, client: ClientId, waker: Option<EventWaker>) {
        self.handle.set_waker(client, waker);
    }

    fn drain_events(&mut self, client: ClientId, out: &mut Vec<ClientEvent>) {
        self.handle.drain_events(client, out);
    }

    fn stats(&self) -> DriverStats {
        self.handle.stats()
    }

    fn memory(&self) -> CompositorMemory {
        self.handle.memory()
    }

    fn output_status(&self, output: OutputId) -> Option<OutputStatus> {
        self.handle.output_status(output)
    }

    fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        self.handle.extra_stats(out);
    }

    fn output_extra_stats(&self, output: OutputId, out: &mut dyn FnMut(&'static str, u64)) {
        self.handle.output_extra_stats(output, out);
    }

    fn trim(&mut self, to_bytes: u64) {
        self.handle.trim(to_bytes);
    }

    fn forget_caches(&mut self) {
        self.handle.forget_caches();
    }

    fn set_verification(&mut self, on: bool) {
        self.handle.set_verification(on);
    }

    fn set_budgets(&mut self, budgets: CompositorBudgets) {
        self.handle.set_budgets(budgets);
    }

    fn step(&mut self, clock: FrameClock) {
        self.handle.step(clock);
    }

    fn pump(&mut self) {
        self.handle.pump();
    }

    fn sync(&mut self) {
        self.handle.sync();
    }
}
