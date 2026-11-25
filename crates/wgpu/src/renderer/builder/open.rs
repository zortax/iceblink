//! Opening the first device a presenter accepts.

use std::ffi::CStr;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use iceblink::GpuUnavailable;

use crate::gpu::adapter;
use crate::gpu::device::Gpu;
use crate::present::native::SwapChain;
use crate::present::{Offscreen, WgpuPresenter};
use crate::renderer::builder::Proof;
use crate::renderer::shared::DeviceState;

/// Opens a device for a renderer rebuilt after a device loss.
pub(crate) fn open_offscreen_device(
    backends: wgpu::Backends,
) -> Result<Rc<DeviceState>, GpuUnavailable> {
    let instances = adapter::Instances::new(backends);
    let swap_chain = SwapChain::Wgpu.overridden();
    let (state, _) = open_device(&instances, &[], None, swap_chain, |gpu: &Arc<Gpu>| {
        Ok(Offscreen::kept(
            gpu,
            [1, 1],
            wgpu::TextureFormat::Rgba8Unorm,
        ))
    })?;
    Ok(state)
}

/// Opens the first device `present` accepts, and returns it with the presenter made on it.
///
/// Adapters are tried in preference order on each tier. An adapter that has the Vulkan device
/// `extensions` and refused a device with them is tried without them after every other one, and a
/// software adapter is tried last.
///
/// Fails with every adapter tried and why it was rejected.
pub(crate) fn open_device<P: Proof>(
    instances: &adapter::Instances,
    extensions: &[&'static CStr],
    cache: Option<&Path>,
    swap_chain: SwapChain,
    mut present: P,
) -> Result<(Rc<DeviceState>, Box<dyn WgpuPresenter>), GpuUnavailable> {
    let mut rejections: Vec<(String, String)> = Vec::new();
    let mut enumerated = 0usize;
    let mut deferred: Vec<(wgpu::Instance, wgpu::Adapter)> = Vec::new();
    let mut software: Vec<(wgpu::Instance, wgpu::Adapter)> = Vec::new();
    let asked = !extensions.is_empty();
    let mut backends = wgpu::Backends::empty();

    for (tier, instance) in instances.iter() {
        backends |= tier;
        present.tier(instance, tier);
        let mut candidates: Vec<(bool, wgpu::Adapter)> = adapter::candidates(instance, tier)
            .into_iter()
            .map(|candidate| {
                (
                    asked && crate::gpu::extensions::granted_by(&candidate, extensions),
                    candidate,
                )
            })
            .collect();
        enumerated += candidates.len();
        // Stable, so the preference order survives inside each group.
        candidates.sort_by_key(|(grants, _)| !grants);
        for (grants, candidate) in candidates {
            let info = candidate.get_info();
            let name = adapter::describe(&info);
            let named = adapter::preferred_device_id() == Some(info.device);
            if adapter::is_software(&info) && !named {
                software.push((instance.clone(), candidate));
                continue;
            }
            let again = grants.then(|| (instance.clone(), candidate.clone()));
            let gpu = match Gpu::open_for(instance.clone(), candidate, extensions, swap_chain) {
                Ok(gpu) => Arc::new(gpu),
                Err(reason) => {
                    rejections.push((name, reason));
                    continue;
                }
            };
            if grants && gpu.vulkan_extensions().is_empty() {
                rejections.push((
                    name,
                    "the driver refused a device with the Vulkan device extensions asked for"
                        .to_owned(),
                ));
                deferred.extend(again);
                continue;
            }
            match offer(&gpu, &mut present) {
                Ok(presenter) => {
                    tracing::info!(adapter = %gpu.describe(), "graphics device opened");
                    return Ok(settled::<P>(&gpu, presenter, cache));
                }
                Err(reason) => rejections.push((name, reason)),
            }
        }
    }

    let fallbacks = deferred
        .into_iter()
        .map(|(instance, candidate)| {
            let warning = "graphics device opened without the Vulkan device extensions asked for";
            (instance, candidate, &[][..], warning)
        })
        .chain(software.into_iter().map(|(instance, candidate)| {
            let warning = "no graphics card could be drawn on, so every pixel is drawn on the \
                           processor";
            (instance, candidate, extensions, warning)
        }));
    for (instance, candidate, extensions, warning) in fallbacks {
        let name = adapter::describe(&candidate.get_info());
        let gpu = match Gpu::open_for(instance, candidate, extensions, swap_chain) {
            Ok(gpu) => Arc::new(gpu),
            Err(reason) => {
                rejections.push((name, reason));
                continue;
            }
        };
        match offer(&gpu, &mut present) {
            Ok(presenter) => {
                tracing::warn!(adapter = %gpu.describe(), "{warning}");
                return Ok(settled::<P>(&gpu, presenter, cache));
            }
            Err(reason) => rejections.push((name, reason)),
        }
    }

    if enumerated == 0 {
        tracing::warn!(backends = ?backends, "no graphics adapter was found");
    }
    Err(adapter::unavailable(rejections))
}

/// Builds the pipelines of `gpu` while `presenter` settles, and returns both.
pub(super) fn settled<P: Proof>(
    gpu: &Arc<Gpu>,
    mut presenter: P::Presenter,
    cache: Option<&Path>,
) -> (Rc<DeviceState>, Box<dyn WgpuPresenter>) {
    let state = DeviceState::new(Arc::clone(gpu), cache);
    let formats = presenter.formats();
    P::settle(gpu, &mut presenter, || state.prepare(formats));
    (state, Box::new(presenter))
}

/// Makes the presenter of `present` on `gpu`.
///
/// Fails with the reason `present` gives. Outside a browser, it also fails with a validation
/// error raised while the presenter is made.
pub(super) fn offer<P: Proof>(gpu: &Arc<Gpu>, present: &mut P) -> Result<P::Presenter, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let scope = gpu.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let presenter = present.present(gpu);
        match (presenter, futures::executor::block_on(scope.pop())) {
            (Ok(presenter), None) => Ok(presenter),
            (Ok(_), Some(error)) => Err(error.to_string()),
            (Err(reason), _) => Err(reason),
        }
    }
    // A browser reports validation errors only once this thread returns to its event loop.
    #[cfg(target_arch = "wasm32")]
    {
        present.present(gpu)
    }
}
