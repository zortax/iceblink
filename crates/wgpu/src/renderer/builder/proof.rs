//! Presenters made on a candidate device, which prove that the device presents.

use std::sync::Arc;

use crate::gpu::device::Gpu;
use crate::present::native::{NativeWindow, SwapChain};
use crate::present::{PresentPacing, SurfacePresenter, WgpuPresenter};

/// Makes a presenter on a device, which proves the device presents.
pub(crate) trait Proof {
    type Presenter: WgpuPresenter + 'static;

    /// Prepares for the adapters of `tier` on `instance`, before any of them is tried.
    fn tier(&mut self, instance: &wgpu::Instance, tier: wgpu::Backends) {
        let _ = (instance, tier);
    }

    /// Makes the presenter on `gpu`, or returns why `gpu` cannot present.
    fn present(&mut self, gpu: &Arc<Gpu>) -> Result<Self::Presenter, String>;

    /// Finishes a presenter while `prepare` builds the device's pipelines.
    fn settle(gpu: &Arc<Gpu>, presenter: &mut Self::Presenter, prepare: impl FnOnce()) {
        let _ = (gpu, presenter);
        prepare();
    }
}

impl<P: WgpuPresenter + 'static, F: FnMut(&Arc<Gpu>) -> Result<P, String>> Proof for F {
    type Presenter = P;

    fn present(&mut self, gpu: &Arc<Gpu>) -> Result<P, String> {
        self(gpu)
    }
}

/// A window to present to, with a surface made on the instance of the tier being tried.
pub(crate) struct WindowSurface<W> {
    pub(crate) window: W,
    pub(crate) size: [u32; 2],
    pub(crate) opaque: bool,
    pub(crate) pacing: PresentPacing,
    pub(crate) swap_chain: SwapChain,
    pub(crate) made: Option<(wgpu::Backends, wgpu::Surface<'static>)>,
}

impl<W: Into<wgpu::SurfaceTarget<'static>> + Clone> Proof for WindowSurface<W> {
    type Presenter = SurfacePresenter;

    fn tier(&mut self, instance: &wgpu::Instance, tier: wgpu::Backends) {
        // A failure here is reported by `present`, which tries again on the adapter's instance.
        self.made = instance
            .create_surface(self.window.clone())
            .ok()
            .map(|surface| (tier, surface));
    }

    fn present(&mut self, gpu: &Arc<Gpu>) -> Result<SurfacePresenter, String> {
        let backend = wgpu::Backends::from(gpu.adapter().get_info().backend);
        let surface = match self.made.take() {
            Some((tier, surface)) if tier.contains(backend) => surface,
            _ => gpu
                .instance()
                .create_surface(self.window.clone())
                .map_err(|error| error.to_string())?,
        };
        if surface.get_capabilities(gpu.adapter()).formats.is_empty() {
            return Err("the surface is not compatible with this adapter".to_owned());
        }

        let mut presenter =
            SurfacePresenter::unconfigured(gpu, surface, self.size, self.opaque, self.pacing);
        presenter.set_window(NativeWindow::of(self.window.clone()));
        presenter.set_swap_chain(self.swap_chain);
        Ok(presenter)
    }

    // Configuring a Vulkan surface on Linux waits on the compositor, so it overlaps the pipeline
    // builds on a thread of its own.
    fn settle(gpu: &Arc<Gpu>, presenter: &mut SurfacePresenter, prepare: impl FnOnce()) {
        let threaded =
            cfg!(target_os = "linux") && gpu.adapter().get_info().backend == wgpu::Backend::Vulkan;
        if !threaded {
            prepare();
            presenter.configure_now();
            return;
        }
        std::thread::scope(|threads| {
            let configuring = threads.spawn(|| presenter.configure_now());
            prepare();
            if let Err(panic) = configuring.join() {
                std::panic::resume_unwind(panic);
            }
        });
    }
}
