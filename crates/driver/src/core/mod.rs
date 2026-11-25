//! The driver core, which applies clients' calls to a compositor and runs its outputs' frames.

mod clients;
mod events;
mod frames;
mod motion;
mod outputs;
mod watch;

use std::collections::HashMap;
use std::sync::Arc;

use iceblink::{
    ClientId, Compositor, CompositorBudgets, CompositorCapabilities, CompositorEvent,
    CompositorMemory, FontRegistry, OutputConfig, OutputId, now_ns,
};

use crate::mailbox::Letter;
use crate::options::{DriverOptions, Pacing};
use crate::output::Output;

use self::events::Watches;

/// A compositor and the renderers of its outputs, driven through the calls of the client seam.
///
/// Every call applies immediately. The events a call causes are addressed to their clients as
/// [`Letter`]s, which [`letters`](Self::letters) returns for delivery. Frames are built by
/// [`run_due`](Self::run_due) for a paced driver and by [`step`](Self::step) for a stepped one.
pub struct Driver {
    compositor: Box<dyn Compositor>,
    options: DriverOptions,
    outputs: Vec<Output>,
    /// The outputs whose renderer could not be built, which draw nothing.
    failed: Vec<OutputId>,
    /// The outputs that wait for a renderer, and their configuration.
    unattached: Vec<(OutputId, OutputConfig)>,
    clients: HashMap<ClientId, Watches>,
    letters: Vec<Letter>,
    /// The buffer the compositor's events are drained into, kept for its allocation.
    events: Vec<CompositorEvent>,
    frames_built: u64,
    steps: u64,
}

impl Driver {
    /// Returns a driver of `compositor`, paced as `options` say.
    pub fn new(compositor: Box<dyn Compositor>, options: DriverOptions) -> Self {
        Self {
            compositor,
            options,
            outputs: Vec::new(),
            failed: Vec::new(),
            unattached: Vec::new(),
            clients: HashMap::new(),
            letters: Vec::new(),
            events: Vec::new(),
            frames_built: 0,
            steps: 0,
        }
    }

    /// Returns the options the driver was made with.
    pub fn options(&self) -> DriverOptions {
        self.options
    }

    /// Returns the capabilities of the compositor.
    pub fn capabilities(&self) -> CompositorCapabilities {
        self.compositor.capabilities()
    }

    /// Returns the compositor's font registry.
    pub fn font_registry(&self) -> Arc<dyn FontRegistry> {
        self.compositor.font_registry()
    }

    /// Evicts unpinned atlas entries until the atlas pages hold at most `to_bytes`.
    pub fn trim(&mut self, to_bytes: u64) {
        self.compositor.trim(to_bytes);
    }

    /// Switches the compositor's verification mode on or off.
    pub fn set_verification(&mut self, on: bool) {
        self.compositor.set_verification(on);
    }

    /// Sets the soft limits of the compositor's caches.
    pub fn set_budgets(&mut self, budgets: CompositorBudgets) {
        self.compositor.set_budgets(budgets);
    }

    /// Drops every atlas entry and lowered chunk, pinned ones included.
    pub fn forget_caches(&mut self) {
        self.compositor.forget_caches();
        let now = self.now();
        self.ask_all(now);
    }

    /// Returns the memory the compositor holds.
    pub fn memory(&self) -> CompositorMemory {
        self.compositor.memory()
    }

    /// Calls `out` with each implementation-specific counter of the compositor, by name.
    pub fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        self.compositor.extra_stats(out);
    }

    /// Returns the letters addressed since the last delivery, oldest first.
    pub fn letters(&mut self) -> &mut Vec<Letter> {
        &mut self.letters
    }

    fn ask_all(&mut self, now_ns: u64) {
        for output in &mut self.outputs {
            output.ask(now_ns);
        }
    }

    /// Returns the time a paced driver schedules from, which a stepped driver does not read.
    fn now(&self) -> u64 {
        match self.options.pacing {
            Pacing::Paced => now_ns(),
            Pacing::Stepped => 0,
        }
    }

    fn output(&mut self, id: OutputId) -> Option<&mut Output> {
        self.outputs.iter_mut().find(|output| output.id == id)
    }
}
