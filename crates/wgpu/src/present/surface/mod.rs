//! A presenter of a window surface, in kept-target mode.

mod acquire;
mod crop;
mod pacing;
mod presenter;

use std::sync::Arc;

use crate::gpu::device::Gpu;
use crate::gpu::formats::Formats;
use crate::present::acquisition::Acquisition;
use crate::present::fault::FaultInjector;
use crate::present::native::{ChainConfig, NativeSwapchain, NativeWindow, SwapChain};

pub(crate) use self::crop::cropped_extent;
pub use self::pacing::PresentPacing;
pub(crate) use self::pacing::present_mode;

/// How many frames may be queued for display.
pub const FRAME_LATENCY: u32 = 2;

/// How long a configuration may block before it is logged.
// A wait this long is visible to a person.
const HUMAN_VISIBLE_WAIT: std::time::Duration = std::time::Duration::from_millis(250);

/// A presenter of a window surface, in kept-target mode.
///
/// The surface presents through a native chain where the [`SwapChain`] chosen asks for one and the
/// device can make one, and through wgpu's swap chain elsewhere. Every image of wgpu's swap chain is
/// uninitialised, so each one owes the whole target. An image of a native chain keeps its texels
/// and owes only its debt.
///
/// A resize is applied when the next image is acquired, after the frame was composed.
#[derive(Debug)]
pub struct SurfacePresenter {
    // Dropped before `surface`, since a swap chain may not outlive its surface. While it exists,
    // the wgpu surface is never configured.
    native: Option<NativeSwapchain>,
    gpu: Arc<Gpu>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    formats: Formats,
    configured: bool,
    /// Whether the next acquisition configures the surface again.
    reconfigure: bool,
    /// The extent the next configuration applies.
    pending: Option<[u32; 2]>,
    /// The extent of the output, at most the configured one.
    used: [u32; 2],
    window: NativeWindow,
    swap_chain: SwapChain,
    /// Whether a native chain was tried.
    native_tried: bool,
    /// The image acquired from wgpu's swap chain, with its id and view.
    acquired: Option<(u32, wgpu::SurfaceTexture, wgpu::TextureView)>,
    /// The present times taken from the native chain.
    // Kept to reuse the allocation.
    presented: Vec<(u64, u64)>,
    /// The id of the last image acquired from wgpu's swap chain.
    sequence: u32,
    faults: FaultInjector,
    /// How long the last acquisition waited.
    block: std::time::Duration,
    crop: bool,
}

impl SurfacePresenter {
    /// Returns a presenter of `surface` on `gpu`, configured for `size`.
    pub fn new(
        gpu: &Arc<Gpu>,
        surface: wgpu::Surface<'static>,
        size: [u32; 2],
        opaque: bool,
        pacing: PresentPacing,
    ) -> Self {
        let mut presenter = Self::unconfigured(gpu, surface, size, opaque, pacing);
        presenter.configure_now();
        presenter
    }

    /// Returns a presenter of `surface` that configures the surface at its first acquisition or
    /// at [`SurfacePresenter::configure_now`].
    ///
    /// The surface presents through the swap chains of [`SwapChain::Auto`], or those
    /// [`PRESENTER_VARIABLE`](crate::present::native::PRESENTER_VARIABLE) chooses.
    pub fn unconfigured(
        gpu: &Arc<Gpu>,
        surface: wgpu::Surface<'static>,
        size: [u32; 2],
        opaque: bool,
        pacing: PresentPacing,
    ) -> Self {
        let capabilities = surface.get_capabilities(gpu.adapter());
        let mode = present_mode(pacing, &capabilities);
        tracing::info!(
            offered = ?capabilities.present_modes,
            chosen = ?mode,
            ?pacing,
            "surface presentation modes"
        );
        let formats = Formats::choose(
            &capabilities.formats,
            &capabilities.alpha_modes,
            opaque,
            gpu.capabilities().mutable_texture_formats,
        );
        formats.log(&gpu.describe());
        debug_assert!(
            formats.is_sound(),
            "nothing cancels the encode of {formats:?}"
        );
        // A swap chain larger than the largest texture fails validation.
        let largest = gpu.capabilities().max_texture_size;
        let used = [size[0].clamp(1, largest), size[1].clamp(1, largest)];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: formats.surface,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: used[0],
            height: used[1],
            present_mode: mode,
            desired_maximum_frame_latency: FRAME_LATENCY,
            alpha_mode: formats.alpha_mode,
            view_formats: formats.view_formats(),
        };
        Self {
            native: None,
            gpu: Arc::clone(gpu),
            surface,
            config,
            formats,
            configured: false,
            reconfigure: true,
            pending: None,
            presented: Vec::new(),
            used,
            window: NativeWindow::default(),
            swap_chain: SwapChain::Auto.overridden(),
            native_tried: false,
            acquired: None,
            sequence: 0,
            faults: FaultInjector::from_environment(),
            block: std::time::Duration::ZERO,
            crop: false,
        }
    }

    /// Configures the surface for its pending size now.
    pub fn configure_now(&mut self) {
        self.apply();
    }

    /// Returns the surface.
    pub fn surface(&self) -> &wgpu::Surface<'static> {
        &self.surface
    }

    /// Returns the presentation mode the surface is configured with.
    pub fn present_mode(&self) -> wgpu::PresentMode {
        self.config.present_mode
    }

    /// Returns how long the last acquisition waited.
    pub fn acquire_block(&self) -> std::time::Duration {
        self.block
    }

    /// Queues `answer` in place of the next `times` acquisitions.
    pub fn inject_fault(&mut self, answer: Acquisition, times: u32) {
        self.faults.inject(answer, times);
    }

    /// Sets whether the surface may be larger than the output, rounded to size classes so a small
    /// resize keeps the configuration.
    ///
    /// Only a platform that shows the output's extent of a larger image can crop.
    pub fn set_crop(&mut self, crop: bool) {
        self.crop = crop;
    }

    /// Reports whether the surface presents through a native chain.
    pub fn is_native(&self) -> bool {
        self.native.is_some()
    }

    /// Sets the handles of the window the surface was made for, which a native chain needs on
    /// some platforms.
    ///
    /// The handles take effect at the first configuration.
    pub(crate) fn set_window(&mut self, window: NativeWindow) {
        self.window = window;
    }

    /// Sets who owns the swap chain.
    ///
    /// The choice takes effect at the first configuration.
    pub(crate) fn set_swap_chain(&mut self, swap_chain: SwapChain) {
        self.swap_chain = swap_chain;
    }

    /// Configures the surface when a resize is pending or a configuration is owed.
    fn apply(&mut self) {
        let resized = self.pending.take();
        if resized.is_none() && self.configured && !self.reconfigure {
            return;
        }
        if let Some([width, height]) = resized {
            self.config.width = width;
            self.config.height = height;
        }
        let began = web_time::Instant::now();
        self.configure_chain();
        let elapsed = began.elapsed();
        if elapsed >= HUMAN_VISIBLE_WAIT {
            tracing::warn!(
                elapsed_ms = elapsed.as_millis() as u64,
                width = self.config.width,
                height = self.config.height,
                "configuring the surface blocked the event-loop thread"
            );
        }
        self.configured = true;
        self.reconfigure = false;
    }

    /// Configures the native chain, made on the first call where one is chosen, or wgpu's swap
    /// chain where there is none.
    fn configure_chain(&mut self) {
        if !self.native_tried {
            self.native_tried = true;
            if self.swap_chain.makes_native(&self.gpu) {
                self.native = NativeSwapchain::new(&self.gpu, &self.surface, self.window);
                if self.native.is_none() {
                    tracing::info!(
                        "this backend or surface cannot have a native swap chain, so wgpu's is used"
                    );
                }
            }
        }
        let chain = ChainConfig::of(&self.config);
        match self
            .native
            .as_mut()
            .map(|native| native.configure(&chain, self.used))
        {
            Some(Ok(())) => {}
            Some(Err(reason)) => {
                tracing::warn!(%reason, "the native swap chain could not be made, so wgpu's is used");
                // Destroyed first: a surface has at most one swap chain.
                self.native = None;
                self.surface.configure(self.gpu.device(), &self.config);
            }
            None => self.surface.configure(self.gpu.device(), &self.config),
        }
    }
}
