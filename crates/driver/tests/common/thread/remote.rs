//! A `Compositor` that forwards to a threaded compositor through its handle, for suites written
//! against the trait.

use std::sync::Arc;

use iceblink::smallvec::SmallVec;
use iceblink::{
    Built, ClientId, CommitError, CompositorEvent, Counters, Frame, FrameId, FrameOutcome,
    FrameRequest, HitResult, IdSpace, OutputId, ScrollGesture, ScrollRouting, Transaction,
};
use iceblink::{
    ClientEvent, CommitReport, Compositor, CompositorBudgets, CompositorCapabilities,
    CompositorClient, CompositorMemory, FontRegistry, GlyphRaster, OutputConfig,
};
use iceblink_driver::thread::{ClientHandle, ThreadedCompositor};

use crate::common::thread::probe::Probe;
use crate::common::thread::scene::{PERIOD_NS, spawn};
use crate::common::thread::watched::Watched;

/// The reference compositor on a driver thread, reached only through a `ClientHandle`.
///
/// Commits, gestures and hit tests wait for the driver's answer. The driver builds and draws the
/// frames, so `build_frame` returns `Built::Idle` and `frame_done` does nothing. The calls the
/// handle does not offer panic.
pub(crate) struct Remote {
    handle: ClientHandle,
    clients: Vec<ClientId>,
    /// The compositor events drained while waiting for answers, oldest first.
    pending: Vec<CompositorEvent>,
    counters: Counters,
    probe: Arc<Probe>,
    // Dropped last, so the driver stops after the handle's last call.
    _driver: ThreadedCompositor,
}

impl Remote {
    pub(crate) fn new() -> Self {
        let (driver, handle) = spawn();
        Self {
            handle,
            clients: Vec::new(),
            pending: Vec::new(),
            counters: Counters::default(),
            probe: Probe::new(),
            _driver: driver,
        }
    }
}

impl Remote {
    /// Waits for the driver, keeps the compositor events of every client, and returns the
    /// other events.
    fn settle(&mut self) -> Vec<ClientEvent> {
        self.handle.sync();
        let mut events = Vec::new();
        for &client in &self.clients {
            self.handle.drain_events(client, &mut events);
        }
        let mut answers = Vec::new();
        for event in events {
            match event {
                ClientEvent::Compositor(event) => self.pending.push(event),
                other => answers.push(other),
            }
        }
        answers
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            assert!(self.probe.state().violations.is_empty());
        }
    }
}

impl Compositor for Remote {
    fn capabilities(&self) -> CompositorCapabilities {
        self.handle.capabilities()
    }

    fn register_client(&mut self) -> (ClientId, IdSpace) {
        let (client, space) = self.handle.register_client();
        self.clients.push(client);
        (client, space)
    }

    fn unregister_client(&mut self, client: ClientId) {
        self.clients.retain(|held| *held != client);
        self.handle.unregister_client(client);
    }

    fn transaction(&mut self, client: ClientId) -> Transaction {
        self.handle.transaction(client)
    }

    fn commit(&mut self, transaction: Transaction) -> Result<CommitReport, CommitError> {
        let id = self.handle.commit(transaction);
        let answers = self.settle();
        self.counters = self.handle.stats().counters;
        answers
            .into_iter()
            .find_map(|event| match event {
                ClientEvent::Committed { commit, report } if commit == id => Some(Ok(report)),
                ClientEvent::Rejected { commit, error } if commit == id => Some(Err(error)),
                _ => None,
            })
            .expect("the commit is answered")
    }

    fn use_font_registry(&mut self, _: Arc<dyn FontRegistry>) {
        unimplemented!("fonts are set before the compositor is spawned")
    }

    fn font_registry(&self) -> Arc<dyn FontRegistry> {
        unimplemented!("fonts are set before the compositor is spawned")
    }

    fn use_glyph_raster(&mut self, _: Arc<dyn GlyphRaster>) {
        unimplemented!("the rasteriser is set before the compositor is spawned")
    }

    fn create_output(&mut self, config: OutputConfig) -> OutputId {
        self.handle
            .create_output(config, Watched::factory(&self.probe, PERIOD_NS))
    }

    fn configure_output(&mut self, output: OutputId, config: OutputConfig) {
        self.handle.configure_output(output, config);
    }

    fn destroy_output(&mut self, output: OutputId) {
        self.handle.destroy_output(output);
    }

    fn build_frame(&mut self, _: OutputId, _: &FrameRequest<'_>, _: &mut Frame) -> Built {
        Built::Idle { wake_ns: None }
    }

    fn frame_done(&mut self, _: OutputId, _: FrameId, _: &FrameOutcome) {}

    fn scroll_input(&mut self, output: OutputId, gesture: ScrollGesture) -> ScrollRouting {
        let client = *self.clients.first().expect("a client hands the gesture");
        let predicted = ScrollRouting::Unhandled;
        self.handle.scroll_input(client, output, gesture, predicted);
        self.settle()
            .into_iter()
            .find_map(|event| match event {
                ClientEvent::Rerouted { routing, .. } => Some(routing),
                _ => None,
            })
            .unwrap_or(predicted)
    }

    fn hit_test(&self, output: OutputId, point: [f32; 2]) -> SmallVec<[HitResult; 4]> {
        self.handle.clone().hit_test(output, point)
    }

    fn drain_events(&mut self, out: &mut Vec<CompositorEvent>) {
        self.settle();
        out.append(&mut self.pending);
    }

    fn set_verification(&mut self, _: bool) {
        unimplemented!("verification is set before the compositor is spawned")
    }

    fn counters(&self) -> &Counters {
        &self.counters
    }

    fn memory(&self) -> CompositorMemory {
        unimplemented!("the handle does not report memory")
    }

    fn set_budgets(&mut self, _: CompositorBudgets) {
        unimplemented!("budgets are set before the compositor is spawned")
    }

    fn budgets(&self) -> CompositorBudgets {
        unimplemented!("the handle does not report budgets")
    }

    fn trim(&mut self, _: u64) -> u64 {
        unimplemented!("the handle does not trim")
    }

    fn forget_caches(&mut self) -> u64 {
        unimplemented!("the handle does not forget caches")
    }
}
