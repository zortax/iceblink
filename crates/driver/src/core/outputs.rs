//! The driver's calls on outputs and their renderers.

use iceblink::{
    ClientEvent, ClientId, FrameObserver, GpuUnavailable, MemoryReport, OutputConfig, OutputId,
    OutputStatus, OutputWatch, RenderCapabilities, Renderer, RendererTask, SubpixelOrder,
    TargetPoolReport, VectorStatus,
};

use crate::core::Driver;
use crate::mailbox::Letter;
use crate::output::Output;

impl Driver {
    /// Adds an output drawn by `renderer`, configured for `config`, and returns its id.
    ///
    /// The output builds no frame until a commit is applied or a frame is requested for it.
    ///
    /// If no renderer could be built, the output draws nothing and every client gets an
    /// `OutputFailed` event.
    pub fn create_output(
        &mut self,
        config: OutputConfig,
        renderer: Result<Box<dyn Renderer>, GpuUnavailable>,
    ) -> OutputId {
        let id = self.add_output(config);
        self.attach_renderer(id, renderer);
        id
    }

    /// Adds an output configured for `config` without a renderer, and returns its id.
    ///
    /// The output waits for the renderer that [`attach_renderer`](Self::attach_renderer)
    /// attaches.
    pub fn add_output(&mut self, config: OutputConfig) -> OutputId {
        let id = self.compositor.create_output(config);
        self.unattached.push((id, config));
        id
    }

    /// Attaches the renderer that draws an output added by [`add_output`](Self::add_output), as
    /// [`create_output`](Self::create_output) does.
    ///
    /// A renderer for an output that is not waiting for one is dropped.
    pub fn attach_renderer(
        &mut self,
        output: OutputId,
        renderer: Result<Box<dyn Renderer>, GpuUnavailable>,
    ) {
        let Some(index) = self.unattached.iter().position(|(id, _)| *id == output) else {
            return;
        };
        let (id, config) = self.unattached.swap_remove(index);
        match renderer {
            Ok(renderer) => self
                .outputs
                .push(Output::new(id, renderer, config, &self.options)),
            Err(error) => {
                self.failed.push(id);
                for &client in self.clients.keys() {
                    let event = ClientEvent::OutputFailed {
                        output: id,
                        error: error.clone(),
                    };
                    self.letters.push(Letter {
                        client,
                        event,
                        wakes: true,
                    });
                }
            }
        }
    }

    /// Changes an output's size, scale or opacity, and configures its renderer to match.
    ///
    /// Once the output's frames have started, the change takes effect with the next applied
    /// commit or [`request_frame`](Self::request_frame) of the output, and a later change replaces
    /// it. The output's frames keep the configuration before it until then.
    pub fn configure_output(&mut self, output: OutputId, config: OutputConfig) {
        if self.output(output).is_some_and(|held| held.defer(config)) {
            return;
        }
        self.apply_configuration(output, config);
    }

    /// Applies the configurations of outputs that wait for a commit or a frame request, of every
    /// output or of `only`.
    pub(crate) fn apply_deferred(&mut self, only: Option<OutputId>) {
        let now = self.now();
        for output in &mut self.outputs {
            if only.is_some_and(|only| only != output.id) {
                continue;
            }
            if let Some(config) = output.take_deferred() {
                self.compositor.configure_output(output.id, config);
                output.configure(config, now);
            }
        }
    }

    fn apply_configuration(&mut self, output: OutputId, config: OutputConfig) {
        self.compositor.configure_output(output, config);
        for (_, waiting) in self.unattached.iter_mut().filter(|(id, _)| *id == output) {
            *waiting = config;
        }
        let now = self.now();
        if let Some(output) = self.output(output) {
            output.configure(config, now);
        }
    }

    /// Sets the stripe order of an output's subpixel sprites, or `None` for none.
    pub fn set_subpixel_order(&mut self, output: OutputId, order: Option<SubpixelOrder>) {
        let now = self.now();
        if let Some(output) = self.output(output) {
            output.set_subpixel_order(order, now);
        }
    }

    /// Sets the refresh interval of the display an output is shown on, or `None` for none known.
    pub fn set_refresh(&mut self, output: OutputId, refresh_ns: Option<u64>) {
        let now = self.now();
        if let Some(output) = self.output(output) {
            output.set_refresh(refresh_ns, now);
        }
    }

    /// Takes `presented_ns`, the time the platform reports a frame of `output` was shown on the
    /// display, as a refresh of the display.
    ///
    /// A stepped driver ignores it.
    pub fn report_presented(&mut self, output: OutputId, presented_ns: u64) {
        let now = self.now();
        if let Some(output) = self.output(output) {
            output.report_presented(presented_ns, now);
        }
    }

    /// Removes an output and drops its renderer.
    pub fn destroy_output(&mut self, output: OutputId) {
        self.compositor.destroy_output(output);
        self.unattached.retain(|(held, _)| *held != output);
        self.outputs.retain(|held| held.id != output);
        self.failed.retain(|held| *held != output);
    }

    /// Wants a frame of an output, which starts its frames and applies a configuration that
    /// waits for it.
    pub fn request_frame(&mut self, output: OutputId) {
        self.apply_deferred(Some(output));
        let now = self.now();
        if let Some(output) = self.output(output) {
            output.start(now);
        }
    }

    /// Sets whether `client` gets a `Drawn` event for every frame drawn for `output`.
    pub fn watch_frames(&mut self, client: ClientId, output: OutputId, on: bool) {
        if let Some(output) = self.output(output) {
            watch(&mut output.watchers, client, on);
        }
    }

    /// Sets the changes of `output` that wake `client`.
    pub fn watch_output(&mut self, client: ClientId, output: OutputId, watch_of: OutputWatch) {
        if let Some(output) = self.output(output) {
            watch(&mut output.shown, client, watch_of.shown);
            watch(&mut output.presenter, client, watch_of.presenter);
        }
    }

    /// Sets the function that sees every frame built for `output`, or removes it with `None`.
    pub fn observe_frames(&mut self, output: OutputId, observer: Option<FrameObserver>) {
        if let Some(output) = self.output(output) {
            output.set_observer(observer);
        }
    }

    /// Runs `task` on the renderer of `output`, if the output has one.
    pub fn with_renderer(&mut self, output: OutputId, task: RendererTask) {
        if let Some(output) = self.output(output) {
            output.run(task);
        }
    }

    /// Frees the pooled targets of the renderer of `output` that are not lent out.
    pub fn release_cached_targets(&mut self, output: OutputId) {
        if let Some(output) = self.output(output) {
            output.release_cached_targets();
        }
    }

    /// Frees the resources the renderer of `output` can recreate.
    pub fn release_idle_resources(&mut self, output: OutputId) {
        if let Some(output) = self.output(output) {
            output.release_idle_resources();
        }
    }

    /// Returns a snapshot of an output, or `None` if the output does not exist or is still
    /// waiting for its renderer.
    pub fn output_status(&self, output: OutputId) -> Option<OutputStatus> {
        if self.failed.contains(&output) {
            return Some(OutputStatus {
                caps: RenderCapabilities::MINIMAL,
                vector: VectorStatus::default(),
                memory: MemoryReport::default(),
                target_pool: TargetPoolReport::default(),
                frames_built: 0,
                failed: true,
                starved: false,
                occluded: false,
            });
        }
        self.outputs
            .iter()
            .find(|held| held.id == output)
            .map(Output::status)
    }

    /// Returns the ids of the outputs that have a renderer, in the order they got it, then those
    /// whose renderer could not be built.
    ///
    /// Outputs still waiting for a renderer are not listed.
    pub fn outputs(&self) -> impl Iterator<Item = OutputId> + '_ {
        self.outputs
            .iter()
            .map(|output| output.id)
            .chain(self.failed.iter().copied())
    }

    /// Calls `out` with each renderer-specific counter of an output's renderer, by name.
    pub fn output_extra_stats(&self, output: OutputId, out: &mut dyn FnMut(&'static str, u64)) {
        if let Some(output) = self.outputs.iter().find(|held| held.id == output) {
            output.extra_stats(out);
        }
    }
}

/// Adds `client` to `watchers` when `on`, and removes it otherwise.
fn watch(watchers: &mut Vec<ClientId>, client: ClientId, on: bool) {
    watchers.retain(|watcher| *watcher != client);
    if on {
        watchers.push(client);
    }
}
