//! The calls of the client seam, queued for the driver thread or answered from its published
//! state.

use std::sync::Arc;
use std::sync::mpsc;

use iceblink::smallvec::SmallVec;
use iceblink::{
    ClientEvent, ClientId, CommitId, CompositorBudgets, CompositorCapabilities, CompositorClient,
    CompositorMemory, DriverPacing, DriverStats, EventWaker, FontRegistry, FrameClock,
    FrameObserver, HitResult, IdSpace, OutputConfig, OutputId, OutputStatus, OutputWatch,
    RendererFactory, RendererTask, ScrollGesture, ScrollId, ScrollRouting, ScrollWatch,
    SubpixelOrder, Transaction,
};

use crate::thread::command::Command;
use crate::thread::handle::ClientHandle;
use crate::{Pacing, now_ns};

impl CompositorClient for ClientHandle {
    fn capabilities(&self) -> CompositorCapabilities {
        self.shared.capabilities
    }

    fn pacing(&self) -> DriverPacing {
        match self.shared.pacing {
            Pacing::Paced => DriverPacing::SelfPaced,
            Pacing::Stepped => DriverPacing::Stepped,
        }
    }

    fn font_registry(&self) -> Arc<dyn FontRegistry> {
        Arc::clone(&self.shared.fonts)
    }

    /// # Panics
    ///
    /// Panics if the call waits and the driver has stopped, or if it waits on the driver thread.
    fn register_client(&mut self) -> (ClientId, IdSpace) {
        let reserved = {
            let mut clients = self.shared.clients();
            let reserved = (!clients.reserve.is_empty()).then(|| clients.reserve.remove(0));
            if let Some((client, space)) = &reserved {
                clients.add(*client, space.clone());
            }
            reserved
        };
        if let Some(reserved) = reserved {
            self.send(Command::Reserve);
            return reserved;
        }

        let (reply, answer) = mpsc::channel();
        self.ask(Command::Register(reply));
        let (client, space) = answer.recv().expect("the compositor thread is running");
        self.shared.clients().add(client, space.clone());
        (client, space)
    }

    fn unregister_client(&mut self, client: ClientId) {
        self.shared.clients().remove(client);
        self.send(Command::Unregister(client));
    }

    fn transaction(&mut self, client: ClientId) -> Transaction {
        self.shared
            .clients()
            .transaction(client)
            .expect("the client was registered through this compositor's handles")
    }

    fn commit(&mut self, transaction: Transaction) -> CommitId {
        let commit = self.shared.next_commit();
        self.send_answered(Command::Commit(commit, transaction));
        commit
    }

    /// # Panics
    ///
    /// Panics if the driver has stopped, or if called on the driver thread.
    fn create_output(&mut self, config: OutputConfig, renderer: RendererFactory) -> OutputId {
        let (reply, answer) = mpsc::channel();
        self.ask(Command::CreateOutput {
            config,
            renderer,
            reply,
        });
        answer.recv().expect("the compositor thread is running")
    }

    fn configure_output(&mut self, output: OutputId, config: OutputConfig) {
        self.send(Command::ConfigureOutput(output, config));
    }

    fn set_subpixel_order(&mut self, output: OutputId, order: Option<SubpixelOrder>) {
        self.send(Command::SetSubpixelOrder(output, order));
    }

    fn set_refresh(&mut self, output: OutputId, refresh_ns: Option<u64>) {
        self.send(Command::SetRefresh(output, refresh_ns));
    }

    fn report_presented(&mut self, output: OutputId, presented_ns: u64) {
        self.send(Command::ReportPresented(output, presented_ns));
    }

    fn destroy_output(&mut self, output: OutputId) {
        self.send(Command::DestroyOutput(output));
    }

    fn request_frame(&mut self, output: OutputId) {
        self.send(Command::RequestFrame(output));
    }

    fn scroll_input(
        &mut self,
        client: ClientId,
        output: OutputId,
        mut gesture: ScrollGesture,
        predicted: ScrollRouting,
    ) {
        if self.shared.pacing == Pacing::Paced {
            gesture.time_ns = now_ns();
        }
        self.send_answered(Command::ScrollInput {
            client,
            output,
            gesture,
            predicted,
        });
    }

    /// The answer is empty once the driver has stopped.
    ///
    /// # Panics
    ///
    /// Panics if called on the driver thread.
    fn hit_test(&mut self, output: OutputId, point: [f32; 2]) -> SmallVec<[HitResult; 4]> {
        let (reply, answer) = mpsc::channel();
        self.ask(Command::HitTest(output, point, reply));
        answer.recv().unwrap_or_default()
    }

    fn watch_scroll(&mut self, client: ClientId, node: ScrollId, watch: Option<ScrollWatch>) {
        self.send(Command::WatchScroll(client, node, watch));
    }

    fn watch_animations(&mut self, client: ClientId, on: bool) {
        self.send(Command::WatchAnimations(client, on));
    }

    fn watch_frames(&mut self, client: ClientId, output: OutputId, on: bool) {
        self.send(Command::WatchFrames(client, output, on));
    }

    fn watch_output(&mut self, client: ClientId, output: OutputId, watch: OutputWatch) {
        self.send(Command::WatchOutput(client, output, watch));
    }

    fn observe_frames(&mut self, output: OutputId, observer: Option<FrameObserver>) {
        self.send(Command::ObserveFrames(output, observer));
    }

    fn with_renderer(&mut self, output: OutputId, task: RendererTask) {
        self.send(Command::WithRenderer(output, task));
    }

    fn release_cached_targets(&mut self, output: OutputId) {
        self.send(Command::ReleaseCachedTargets(output));
    }

    fn release_idle_resources(&mut self, output: OutputId) {
        self.send(Command::ReleaseIdleResources(output));
    }

    fn set_waker(&mut self, client: ClientId, waker: Option<EventWaker>) {
        self.shared.clients().mailbox.set_waker(client, waker);
    }

    fn drain_events(&mut self, client: ClientId, out: &mut Vec<ClientEvent>) {
        self.shared.clients().mailbox.drain(client, out);
    }

    fn stats(&self) -> DriverStats {
        self.shared.snapshot().stats
    }

    fn memory(&self) -> CompositorMemory {
        self.shared.snapshot().memory
    }

    fn output_status(&self, output: OutputId) -> Option<OutputStatus> {
        self.shared
            .snapshot()
            .outputs
            .iter()
            .find(|published| published.id == output)
            .map(|published| published.status)
    }

    fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        for &(name, value) in &self.shared.snapshot().extra {
            out(name, value);
        }
    }

    fn output_extra_stats(&self, output: OutputId, out: &mut dyn FnMut(&'static str, u64)) {
        let snapshot = self.shared.snapshot();
        if let Some(published) = snapshot.outputs.iter().find(|held| held.id == output) {
            for &(name, value) in &published.extra {
                out(name, value);
            }
        }
    }

    fn trim(&mut self, to_bytes: u64) {
        self.send(Command::Trim(to_bytes));
    }

    fn forget_caches(&mut self) {
        self.send(Command::ForgetCaches);
    }

    fn set_verification(&mut self, on: bool) {
        self.send(Command::SetVerification(on));
    }

    fn set_budgets(&mut self, budgets: CompositorBudgets) {
        self.send(Command::SetBudgets(budgets));
    }

    fn step(&mut self, clock: FrameClock) {
        self.send(Command::Step(clock));
    }

    fn pump(&mut self) {}

    /// The driver publishes what the calls changed before the call returns, which it does
    /// immediately once the driver has stopped.
    ///
    /// # Panics
    ///
    /// Panics if called on the driver thread.
    fn sync(&mut self) {
        let (reply, answer) = mpsc::channel();
        if self.ask(Command::Sync(reply)) {
            let _ = answer.recv();
        }
    }
}
