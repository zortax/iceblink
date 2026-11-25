//! Opening shared graphics for a browser canvas without blocking.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use iceblink::GpuUnavailable;

use crate::gpu::adapter;
use crate::gpu::device::Gpu;
use crate::renderer::shared::{DeviceState, Shared, SharedGraphics};

impl SharedGraphics {
    /// Opens the device for `canvas` without blocking the thread while it opens.
    ///
    /// The tiers of `backends` are tried in order: WebGPU first, then WebGL 2. The WebGPU tier
    /// leaves `canvas` untouched. The WebGL 2 tier chooses its adapter with a surface on
    /// `canvas`. Dual-source blending is asked for only when `subpixel_text` is set.
    ///
    /// The graphics behave like those of [`SharedGraphics::with_gpu`]: they open no further
    /// device. After a loss, the application awaits this function again and passes the new
    /// device to [`SharedGraphics::adopt`].
    ///
    /// Fails with one rejection for each tier that was refused.
    pub async fn open_async(
        backends: wgpu::Backends,
        canvas: impl Into<wgpu::SurfaceTarget<'static>> + Clone,
        subpixel_text: bool,
    ) -> Result<Self, GpuUnavailable> {
        let mut unavailable = GpuUnavailable::default();
        for tier in adapter::tiers(backends) {
            let gl = tier.intersects(wgpu::Backends::GL);
            let name = if gl { "WebGL 2" } else { "WebGPU" };
            let instance = adapter::Instances::new(tier).primary().clone();
            // A WebGL adapter belongs to one canvas. A canvas keeps the first context type it
            // gives out, so the WebGPU tier makes no surface here.
            let surface = if gl {
                match instance.create_surface(canvas.clone()) {
                    Ok(surface) => Some(surface),
                    Err(error) => {
                        unavailable = unavailable.rejected(name, error.to_string());
                        continue;
                    }
                }
            } else {
                None
            };
            let options = wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: surface.as_ref(),
                ..Default::default()
            };
            let chosen = match instance.request_adapter(&options).await {
                Ok(chosen) => chosen,
                Err(error) => {
                    unavailable = unavailable.rejected(name, error.to_string());
                    continue;
                }
            };
            // Renderers make their own surfaces on the canvas.
            drop(surface);
            match Gpu::open_async(instance, chosen, subpixel_text).await {
                Ok(gpu) => {
                    let gpu = Arc::new(gpu);
                    return Ok(Self(Rc::new(Shared {
                        instances: adapter::Instances::existing(gpu.instance().clone(), tier),
                        backends: tier,
                        extensions: Vec::new(),
                        cache: None,
                        latency: None,
                        adopted: true,
                        primary: RefCell::new(Some(DeviceState::new(gpu, None))),
                        fallbacks: RefCell::new(Vec::new()),
                    })));
                }
                Err(reason) => unavailable = unavailable.rejected(name, reason),
            }
        }
        Err(unavailable)
    }
}
