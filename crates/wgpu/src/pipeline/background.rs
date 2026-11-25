//! A thread that builds pipelines a renderer will need, while its first frames build their own.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::JoinHandle;

use crate::gpu::device::Gpu;
use crate::pipeline::create::{build, shader_module};
use crate::pipeline::kind::PipelineKind;
use crate::pipeline::layout::Layouts;

/// A pipeline the thread built: its kind, its target format and the pipeline.
pub(crate) type Built = (PipelineKind, wgpu::TextureFormat, wgpu::RenderPipeline);

/// The pipelines a thread builds, in the order they were asked for.
///
/// Dropping it stops the thread after the pipeline it is building, and waits for it.
#[derive(Debug)]
pub(crate) struct Background {
    built: Receiver<Built>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// Reports whether pipelines can be built on a thread of their own on this target.
pub(crate) fn available() -> bool {
    cfg!(not(target_arch = "wasm32"))
}

impl Background {
    /// Starts a thread that builds each kind and target format in `wanted` on `gpu`, through
    /// `cache`.
    pub(crate) fn spawn(
        gpu: Arc<Gpu>,
        layouts: Layouts,
        cache: Option<wgpu::PipelineCache>,
        wanted: Vec<(PipelineKind, wgpu::TextureFormat)>,
    ) -> Self {
        let (send, built) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let spawned = std::thread::Builder::new()
            .name("iceblink-pipelines".to_owned())
            .spawn(move || {
                let mut modules = BTreeMap::new();
                for (kind, format) in wanted {
                    if stopped.load(Ordering::Relaxed) {
                        return;
                    }
                    let source = kind.module();
                    let module = modules
                        .entry(source)
                        .or_insert_with(|| shader_module(&gpu, source));
                    let pipeline = build(&gpu, &layouts, cache.as_ref(), kind, format, module);
                    if send.send((kind, format, pipeline)).is_err() {
                        return;
                    }
                }
            });
        let thread = spawned
            .inspect_err(|error| {
                // Each pipeline is then built when first drawn with.
                tracing::debug!(%error, "the thread that builds pipelines could not be started");
            })
            .ok();
        Self {
            built,
            stop,
            thread,
        }
    }

    /// Returns the pipelines built since the last call, and whether the thread is done.
    pub(crate) fn take(&self) -> (bool, Vec<Built>) {
        let mut built = Vec::new();
        loop {
            match self.built.try_recv() {
                Ok(pipeline) => built.push(pipeline),
                Err(TryRecvError::Empty) => return (false, built),
                Err(TryRecvError::Disconnected) => return (true, built),
            }
        }
    }
}

impl Drop for Background {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            // A driver tears down under a thread that is still compiling and crashes the process.
            let _ = thread.join();
        }
    }
}
