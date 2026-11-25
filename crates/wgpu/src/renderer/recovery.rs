//! Moving a renderer onto a new device after its device was lost.

use std::rc::Rc;
use std::sync::Arc;

use iceblink::GpuUnavailable;
use iceblink::Renderer as _;

use crate::gpu::device::Gpu;
use crate::renderer::WgpuRenderer;
use crate::renderer::builder::{Options, open_offscreen_device};
use crate::renderer::shared::DeviceState;

/// A function that returns the device a renderer moves to after the device it is given was lost,
/// or `None` while the application has no replacement.
pub type ReplacementGpu = Box<dyn FnMut(&Arc<Gpu>) -> Option<Arc<Gpu>>>;

/// Where a renderer built without shared graphics finds a device after a loss.
#[derive(Default)]
pub(crate) enum Recovery {
    /// The renderer opens a device of its own.
    #[default]
    Open,
    /// The renderer asks the application, and has no device while the application has none.
    Ask(Option<ReplacementGpu>),
}

impl WgpuRenderer {
    /// Rebuilds the renderer on a new device after the old one was lost.
    ///
    /// Every setting is kept, and a path rasteriser set through a factory is rebuilt.
    ///
    /// Fails when no replacement device is available or the presenter cannot move onto it.
    pub(crate) fn recover(&mut self) -> Result<(), GpuUnavailable> {
        let state = self.replacement()?;
        self.presenter.rebuild(&state.gpu)?;

        let presenter = std::mem::replace(
            &mut self.presenter,
            Box::new(crate::present::Offscreen::kept(
                &state.gpu,
                [1, 1],
                wgpu::TextureFormat::Rgba8Unorm,
            )),
        );
        let options = Options {
            pre_present: self.pre_present.take(),
            latency: self.latency.clone(),
            precision: Some(self.groups.precision()),
            recovery: std::mem::take(&mut self.recovery),
        };
        let target = self.target;
        let subpixel = self.subpixel_order;
        let gate = self.present_gate.take();
        let groups = self.groups.emptied();
        let (kept, fatal) = (self.rasters_kept, self.vector_shortfall_is_fatal);
        let had_raster = self.vectors.is_some();
        let factory = self.vector_factory.take();
        let (backend, name) = (self.vector_backend, self.vector_name);
        let rebuilt = Self::assemble(
            state,
            self.shared.clone(),
            presenter,
            options,
            self.backends,
        );
        *self = rebuilt;

        self.subpixel_order = subpixel;
        self.present_gate = gate;
        self.groups = groups;
        self.rasters_kept = kept;
        self.vector_shortfall_is_fatal = fatal;
        let rebuilds = factory.is_some();
        self.vector_factory = factory;
        self.vector_backend = backend.filter(|_| rebuilds);
        self.vector_name = if rebuilds { name } else { "" };
        if had_raster && !rebuilds {
            tracing::warn!(
                "the device was rebuilt and the path rasteriser it held did not survive"
            );
        }
        if let Some(target) = target {
            self.configure(target);
        }
        self.full_damage_next = true;
        Ok(())
    }

    /// Returns the device the renderer moves to.
    ///
    /// Fails when no device opens, or the application has no device that is not lost.
    fn replacement(&mut self) -> Result<Rc<DeviceState>, GpuUnavailable> {
        if let Some(shared) = &self.shared {
            return shared.replacement_for(&self.gpu);
        }
        let ask = match &mut self.recovery {
            Recovery::Open => return open_offscreen_device(self.backends),
            Recovery::Ask(ask) => ask,
        };
        match ask.as_mut().and_then(|ask| ask(&self.gpu)) {
            Some(gpu) if !gpu.loss().is_lost() => Ok(DeviceState::new(gpu, None)),
            _ => Err(GpuUnavailable::default().rejected(
                "the application's device",
                "the device is lost and the application has given no replacement yet",
            )),
        }
    }
}
