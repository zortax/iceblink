//! Building a renderer: choosing a device and a presenter, and the renderer's options.

mod open;
mod proof;

use std::path::PathBuf;
use std::sync::Arc;

use iceblink::GpuUnavailable;

use crate::gpu::adapter;
use crate::gpu::device::Gpu;
use crate::present::native::SwapChain;
use crate::present::{Offscreen, PresentPacing, WgpuPresenter};
use crate::renderer::latency::{Latency, LatencyMarks};
use crate::renderer::recovery::{Recovery, ReplacementGpu};
use crate::renderer::shared::DeviceState;
use crate::renderer::{PrePresent, WgpuRenderer};
use crate::target::group_pool::GroupPrecision;

pub(crate) use self::open::{open_device, open_offscreen_device};
pub(crate) use self::proof::{Proof, WindowSurface};

use self::open::offer;

/// The options a renderer is assembled with.
#[derive(Default)]
pub(crate) struct Options {
    pub(crate) pre_present: Option<PrePresent>,
    pub(crate) latency: Latency,
    pub(crate) precision: Option<GroupPrecision>,
    pub(crate) recovery: Recovery,
}

/// Builds a [`WgpuRenderer`] with its presenter.
///
/// Without [`Builder::from_gpu`], the builder opens the first adapter that can present, preferring
/// hardware over software and native backends over GL.
pub struct Builder {
    instances: adapter::Instances,
    backends: wgpu::Backends,
    gpu: Option<Arc<Gpu>>,
    pacing: PresentPacing,
    swap_chain: SwapChain,
    cache: Option<PathBuf>,
    options: Options,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    /// Returns a builder over the backends the environment asks for, or the platform's default.
    pub fn new() -> Self {
        Self::with_backends(adapter::requested_backends())
    }

    /// Returns a builder over `backends`.
    pub fn with_backends(backends: wgpu::Backends) -> Self {
        Self {
            instances: adapter::Instances::new(backends),
            backends,
            gpu: None,
            pacing: PresentPacing::Display,
            swap_chain: SwapChain::Auto,
            cache: None,
            options: Options::default(),
        }
    }

    /// Returns a builder that draws on a device the application opened.
    ///
    /// The renderer never opens a device of its own. Once the device is lost, every draw returns
    /// `Skipped(DeviceUnavailable)` until the function set with [`Builder::with_replacement_gpu`]
    /// returns a device that is not lost and the presenter moves onto it. The draw that moves the
    /// renderer returns `Recovered`.
    ///
    /// A presenter moves when its [`WgpuPresenter::rebuild`] succeeds, as an offscreen
    /// presenter's does. A window surface or supplied textures belong to their owner, so such a
    /// renderer stays unavailable, and the owner builds a new renderer on the replacement.
    pub fn from_gpu(gpu: Arc<Gpu>) -> Self {
        let backends = wgpu::Backends::from(gpu.adapter().get_info().backend);
        Self {
            instances: adapter::Instances::existing(gpu.instance().clone(), backends),
            backends,
            gpu: Some(gpu),
            pacing: PresentPacing::Display,
            swap_chain: SwapChain::Auto,
            cache: None,
            options: Options {
                recovery: Recovery::Ask(None),
                ..Options::default()
            },
        }
    }

    /// Sets the function that supplies the device the renderer moves to after its device was
    /// lost.
    ///
    /// The function is called with the lost device on each draw until the renderer has moved, so
    /// it is asked again after returning a device the presenter could not move onto. While it
    /// returns `None`, draws report `Skipped(DeviceUnavailable)`, which asks for no further frame.
    pub fn with_replacement_gpu(mut self, replace: ReplacementGpu) -> Self {
        self.options.recovery = Recovery::Ask(Some(replace));
        self
    }

    /// Sets what paces a surface presenter.
    pub fn with_present_pacing(mut self, pacing: PresentPacing) -> Self {
        self.pacing = pacing;
        self
    }

    /// Sets who owns a window's swap chain, [`SwapChain::Auto`] by default.
    ///
    /// [`NATIVE_PRESENTER_VARIABLE`](crate::NATIVE_PRESENTER_VARIABLE) overrides the choice when
    /// it is set.
    pub fn with_swap_chain(mut self, swap_chain: SwapChain) -> Self {
        self.swap_chain = swap_chain;
        self
    }

    /// Sets the function called after a frame's work is submitted and before it is presented.
    pub fn with_pre_present(mut self, notify: PrePresent) -> Self {
        self.options.pre_present = Some(notify);
        self
    }

    /// Sets the directory the pipeline cache is kept in.
    ///
    /// Without it, the cache is kept in the directory in
    /// [`PIPELINE_CACHE_VARIABLE`](crate::PIPELINE_CACHE_VARIABLE), or under the user's cache
    /// directory.
    pub fn with_pipeline_cache_dir(mut self, directory: impl Into<PathBuf>) -> Self {
        self.cache = Some(directory.into());
        self
    }

    /// Sets the function called at each step of every frame.
    pub fn with_latency_marks(mut self, marks: LatencyMarks) -> Self {
        self.options.latency = Latency::new(Some(marks));
        self
    }

    /// Sets the precision group targets are held at.
    pub fn with_group_precision(mut self, precision: GroupPrecision) -> Self {
        self.options.precision = Some(precision);
        self
    }

    /// Returns the instance adapters are found on.
    pub fn instance(&self) -> &wgpu::Instance {
        self.instances.primary()
    }

    /// Returns the backends the builder tries.
    pub fn backends(&self) -> wgpu::Backends {
        self.backends
    }

    /// Builds a renderer presenting into one offscreen texture of `format`, in kept-target mode.
    ///
    /// Fails when no adapter can open a device.
    pub fn offscreen(
        self,
        size: [u32; 2],
        format: wgpu::TextureFormat,
    ) -> Result<WgpuRenderer, GpuUnavailable> {
        self.open_windowless(move |gpu: &Arc<Gpu>| Ok(Offscreen::kept(gpu, size, format)))
    }

    /// Builds a renderer presenting into `count` offscreen textures of `format`, in buffer-age
    /// mode.
    ///
    /// Fails when no adapter can open a device.
    pub fn offscreen_buffer_age(
        self,
        size: [u32; 2],
        format: wgpu::TextureFormat,
        count: usize,
    ) -> Result<WgpuRenderer, GpuUnavailable> {
        self.open_windowless(move |gpu: &Arc<Gpu>| {
            Ok(Offscreen::buffer_age(gpu, count, size, format))
        })
    }

    /// Builds a renderer presenting into a window's surface.
    ///
    /// Fails when no adapter can open a device that presents to the window.
    pub fn for_surface(
        self,
        size: [u32; 2],
        opaque: bool,
        window: impl Into<wgpu::SurfaceTarget<'static>> + Clone,
    ) -> Result<WgpuRenderer, GpuUnavailable> {
        let pacing = self.pacing;
        let swap_chain = self.swap_chain.overridden();
        let surface = WindowSurface {
            window,
            size,
            opaque,
            pacing,
            swap_chain,
            made: None,
        };
        self.open(surface, swap_chain)
    }

    /// Builds a renderer presenting through a presenter `present` makes on the chosen device.
    ///
    /// Fails when no adapter opens a device `present` accepts.
    pub fn with_presenter<P: WgpuPresenter + 'static>(
        self,
        present: impl FnMut(&Arc<Gpu>) -> Result<P, String>,
    ) -> Result<WgpuRenderer, GpuUnavailable> {
        self.open_windowless(present)
    }

    /// Opens a device for `present`, which presents into no window.
    // A device without a window makes no swap chain, so it is opened as for wgpu's.
    fn open_windowless<P: Proof>(self, present: P) -> Result<WgpuRenderer, GpuUnavailable> {
        self.open(present, SwapChain::Wgpu.overridden())
    }

    /// Opens the builder's device, or else the first device `present` accepts, and assembles the
    /// renderer on it.
    fn open<P: Proof>(
        self,
        present: P,
        swap_chain: SwapChain,
    ) -> Result<WgpuRenderer, GpuUnavailable> {
        let (state, presenter) = match &self.gpu {
            Some(gpu) => {
                let mut present = present;
                let state = DeviceState::new(Arc::clone(gpu), self.cache.as_deref());
                let mut presenter = offer(gpu, &mut present)
                    .map_err(|reason| GpuUnavailable::default().rejected(gpu.describe(), reason))?;
                let formats = presenter.formats();
                P::settle(gpu, &mut presenter, || state.prepare(formats));
                (state, Box::new(presenter) as Box<dyn WgpuPresenter>)
            }
            None => open_device(
                &self.instances,
                &[],
                self.cache.as_deref(),
                swap_chain,
                present,
            )?,
        };
        Ok(WgpuRenderer::assemble(
            state,
            None,
            presenter,
            self.options,
            self.backends,
        ))
    }
}
