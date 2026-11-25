//! One device shared by every window and offscreen renderer of an application.
//!
//! The first renderer chooses the device. Every later one presents on it when it can, and a
//! window the device cannot present to gets a device of its own. After a device loss, every
//! renderer on the lost device moves to one replacement.

mod device;
mod open_async;
mod surface;

use std::cell::RefCell;
use std::ffi::CStr;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use iceblink::GpuUnavailable;

use crate::gpu::adapter;
use crate::gpu::device::Gpu;
use crate::present::native::SwapChain;
use crate::present::{Offscreen, PresentPacing, WgpuPresenter};
use crate::renderer::builder::{Options, Proof, WindowSurface, open_device};
use crate::renderer::latency::{Latency, LatencyMarks};
use crate::renderer::recovery::Recovery;
use crate::renderer::{PrePresent, WgpuRenderer};

pub(crate) use self::device::DeviceState;

use self::surface::{Rejected, present_on};

/// The device an application's renderers share.
#[derive(Clone)]
pub struct SharedGraphics(Rc<Shared>);

/// The state behind [`SharedGraphics`].
struct Shared {
    instances: adapter::Instances,
    backends: wgpu::Backends,
    /// The Vulkan device extensions devices are opened with.
    extensions: Vec<&'static CStr>,
    /// The directory pipeline caches are kept in.
    cache: Option<PathBuf>,
    latency: Option<LatencyMarks>,
    /// The device new renderers draw on.
    primary: RefCell<Option<Rc<DeviceState>>>,
    /// Whether these graphics draw only on a device the application gave them.
    adopted: bool,
    /// The devices opened for windows the primary device cannot present to.
    fallbacks: RefCell<Vec<Weak<DeviceState>>>,
}

impl Default for SharedGraphics {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedGraphics {
    /// Returns shared graphics over the backends the environment asks for.
    pub fn new() -> Self {
        Self::with_backends_and_extensions(adapter::requested_backends(), Vec::new())
    }

    /// Returns shared graphics over `backends`.
    pub fn with_backends(backends: wgpu::Backends) -> Self {
        Self::with_backends_and_extensions(backends, Vec::new())
    }

    /// Returns shared graphics whose device is opened with the Vulkan device `extensions` where
    /// an adapter has them.
    pub fn with_backends_and_extensions(
        backends: wgpu::Backends,
        extensions: Vec<&'static CStr>,
    ) -> Self {
        Self(Rc::new(Shared {
            instances: adapter::Instances::new(backends),
            backends,
            extensions,
            cache: None,
            latency: None,
            adopted: false,
            primary: RefCell::new(None),
            fallbacks: RefCell::new(Vec::new()),
        }))
    }

    /// Returns shared graphics that draw only on a device the application opened.
    pub fn with_gpu(gpu: Arc<Gpu>) -> Self {
        let backends = wgpu::Backends::from(gpu.adapter().get_info().backend);
        Self(Rc::new(Shared {
            instances: adapter::Instances::existing(gpu.instance().clone(), backends),
            backends,
            extensions: gpu.vulkan_extensions().to_vec(),
            cache: None,
            latency: None,
            adopted: true,
            primary: RefCell::new(Some(DeviceState::new(gpu, None))),
            fallbacks: RefCell::new(Vec::new()),
        }))
    }

    /// Returns these graphics with the pipeline cache kept in `directory` and every renderer
    /// emitting `latency` marks.
    ///
    /// The directory applies to devices opened afterwards, and the marks to renderers made
    /// afterwards.
    #[must_use]
    pub fn with_options(self, directory: Option<PathBuf>, latency: Option<LatencyMarks>) -> Self {
        let shared = &self.0;
        Self(Rc::new(Shared {
            instances: shared.instances.clone(),
            backends: shared.backends,
            extensions: shared.extensions.clone(),
            cache: directory,
            latency,
            adopted: shared.adopted,
            primary: RefCell::new(shared.primary.borrow().clone()),
            fallbacks: RefCell::new(shared.fallbacks.borrow().clone()),
        }))
    }

    /// Makes `gpu` the device new renderers draw on.
    pub fn adopt(&self, gpu: Arc<Gpu>) {
        let mut primary = self.0.primary.borrow_mut();
        if primary
            .as_ref()
            .is_some_and(|state| Arc::ptr_eq(&state.gpu, &gpu))
        {
            return;
        }
        *primary = Some(DeviceState::new(gpu, self.0.cache.as_deref()));
    }

    /// Reports whether these graphics draw only on a device the application gave them.
    pub fn is_adopted(&self) -> bool {
        self.0.adopted
    }

    /// Returns the instance of the first backend tier.
    pub fn instance(&self) -> &wgpu::Instance {
        self.0.instances.primary()
    }

    /// Returns the backends devices are opened on.
    pub fn backends(&self) -> wgpu::Backends {
        self.0.backends
    }

    /// Returns the Vulkan device extensions devices are opened with.
    pub fn extensions(&self) -> &[&'static CStr] {
        &self.0.extensions
    }

    /// Returns the shared device, if one is open.
    pub fn gpu(&self) -> Option<Arc<Gpu>> {
        self.0
            .primary
            .borrow()
            .as_ref()
            .map(|state| Arc::clone(&state.gpu))
    }

    /// Returns the shared device, opening one if none is open or the open one was lost.
    ///
    /// Fails when no adapter opens a device.
    pub fn open_gpu(&self) -> Result<Arc<Gpu>, GpuUnavailable> {
        if let Some(state) = self.usable_primary() {
            return Ok(Arc::clone(&state.gpu));
        }
        Ok(Arc::clone(&self.open_primary()?.gpu))
    }

    /// Returns a renderer presenting into a window's surface, on the shared device when it can
    /// present there.
    ///
    /// Fails when no device presents to the window.
    pub fn renderer_for_surface(
        &self,
        size: [u32; 2],
        opaque: bool,
        window: impl Into<wgpu::SurfaceTarget<'static>> + Clone,
        pacing: PresentPacing,
        pre_present: Option<PrePresent>,
    ) -> Result<WgpuRenderer, GpuUnavailable> {
        let mut surface = WindowSurface {
            window: window.clone(),
            size,
            opaque,
            pacing,
            swap_chain: SwapChain::Auto.overridden(),
            made: None,
        };
        // A lost device is not handed to another window.
        if let Some(state) = self.usable_primary() {
            match present_on(&state, &mut surface) {
                Ok(presenter) => return Ok(self.assemble(state, presenter, pre_present)),
                Err(Rejected::Incompatible) => {
                    return self.on_another_device(surface, pre_present);
                }
                Err(Rejected::Failed(reason)) => {
                    return Err(GpuUnavailable::default().rejected(state.gpu.describe(), reason));
                }
            }
        }
        let (state, presenter) = self.open_one(surface)?;
        *self.0.primary.borrow_mut() = Some(Rc::clone(&state));
        Ok(self.assemble(state, presenter, pre_present))
    }

    /// Returns a renderer on the shared device presenting into an offscreen texture of `format`,
    /// in kept-target mode.
    ///
    /// Fails when no adapter opens a device.
    pub fn renderer_offscreen(
        &self,
        size: [u32; 2],
        format: wgpu::TextureFormat,
    ) -> Result<WgpuRenderer, GpuUnavailable> {
        self.renderer_with_presenter(|gpu: &Arc<Gpu>| Ok(Offscreen::kept(gpu, size, format)))
    }

    /// Returns a renderer on the shared device presenting through what `present` makes.
    ///
    /// Fails when no adapter opens a device `present` accepts.
    pub fn renderer_with_presenter<P: WgpuPresenter + 'static>(
        &self,
        mut present: impl FnMut(&Arc<Gpu>) -> Result<P, String>,
    ) -> Result<WgpuRenderer, GpuUnavailable> {
        if let Some(state) = self.usable_primary() {
            let presenter = present(&state.gpu).map_err(|reason| {
                GpuUnavailable::default().rejected(state.gpu.describe(), reason)
            })?;
            return Ok(self.assemble(state, Box::new(presenter), None));
        }
        let (state, presenter) = self.open_one(present)?;
        *self.0.primary.borrow_mut() = Some(Rc::clone(&state));
        Ok(self.assemble(state, presenter, None))
    }

    /// Returns the device a renderer on the lost device `lost` moves to.
    pub(crate) fn replacement_for(
        &self,
        lost: &Arc<Gpu>,
    ) -> Result<Rc<DeviceState>, GpuUnavailable> {
        if let Some(state) = self.usable_primary()
            && !Arc::ptr_eq(&state.gpu, lost)
        {
            return Ok(state);
        }
        self.open_primary()
    }

    /// Opens a new shared device.
    fn open_primary(&self) -> Result<Rc<DeviceState>, GpuUnavailable> {
        let (state, _) = self.open_one(|gpu: &Arc<Gpu>| {
            Ok(Offscreen::kept(
                gpu,
                [1, 1],
                wgpu::TextureFormat::Rgba8Unorm,
            ))
        })?;
        // Dropping the strong reference releases a dead device once its last renderer moved.
        *self.0.primary.borrow_mut() = Some(Rc::clone(&state));
        Ok(state)
    }

    /// Opens the first device `present` accepts.
    ///
    /// Fails without trying when these graphics draw only on an adopted device.
    fn open_one<P: Proof>(
        &self,
        present: P,
    ) -> Result<(Rc<DeviceState>, Box<dyn WgpuPresenter>), GpuUnavailable> {
        if self.0.adopted {
            // A device opened here would be one the application's resources cannot be used with.
            return Err(GpuUnavailable::default().rejected(
                "the adopted device",
                "these graphics draw only on the device the application gave them, which is lost \
                 or cannot present here; SharedGraphics::adopt gives a new one",
            ));
        }
        open_device(
            &self.0.instances,
            &self.0.extensions,
            self.0.cache.as_deref(),
            SwapChain::Auto.overridden(),
            present,
        )
    }

    /// Returns the shared device, unless none is open or it was lost.
    fn usable_primary(&self) -> Option<Rc<DeviceState>> {
        let primary = self.0.primary.borrow();
        primary
            .as_ref()
            .filter(|state| !state.gpu.loss().is_lost())
            .map(Rc::clone)
    }

    /// Returns a renderer for a window the shared device cannot present to, on a device opened
    /// for such windows before, or else on a new one.
    fn on_another_device<W: Into<wgpu::SurfaceTarget<'static>> + Clone>(
        &self,
        mut surface: WindowSurface<W>,
        pre_present: Option<PrePresent>,
    ) -> Result<WgpuRenderer, GpuUnavailable> {
        self.0.fallbacks.borrow_mut().retain(|state| {
            state
                .upgrade()
                .is_some_and(|state| !state.gpu.loss().is_lost())
        });
        let known: Vec<Rc<DeviceState>> = self
            .0
            .fallbacks
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for state in known {
            match present_on(&state, &mut surface) {
                Ok(presenter) => return Ok(self.assemble(state, presenter, pre_present)),
                Err(Rejected::Incompatible) => {}
                Err(Rejected::Failed(reason)) => {
                    return Err(GpuUnavailable::default().rejected(state.gpu.describe(), reason));
                }
            }
        }
        let (state, presenter) = self.open_one(surface)?;
        self.0.fallbacks.borrow_mut().push(Rc::downgrade(&state));
        Ok(self.assemble(state, presenter, pre_present))
    }

    /// Assembles a renderer on `state` that moves with these graphics after a device loss.
    fn assemble(
        &self,
        state: Rc<DeviceState>,
        presenter: Box<dyn WgpuPresenter>,
        pre_present: Option<PrePresent>,
    ) -> WgpuRenderer {
        let options = Options {
            pre_present,
            latency: Latency::new(self.0.latency.clone()),
            precision: None,
            recovery: Recovery::Open,
        };
        WgpuRenderer::assemble(
            state,
            Some(self.clone()),
            presenter,
            options,
            self.0.backends,
        )
    }
}
