//! The features, limits and Vulkan extensions a device is requested with.

use std::ffi::CStr;

#[cfg(vulkan_hal)]
use crate::gpu::extensions;
use crate::present::native::SwapChain;

/// Returns the descriptor a device on `adapter` is requested with.
///
/// The device asks for the optional features the adapter has, and for the adapter's limits. On
/// GL it asks for the downlevel defaults at the adapter's resolution, lowered to what the adapter
/// offers.
pub(super) fn descriptor(adapter: &wgpu::Adapter) -> wgpu::DeviceDescriptor<'static> {
    let limits = if adapter.get_info().backend == wgpu::Backend::Gl {
        wgpu::Limits::downlevel_defaults()
            .using_resolution(adapter.limits())
            .or_worse_values_from(&adapter.limits())
    } else {
        adapter.limits()
    };
    wgpu::DeviceDescriptor {
        label: Some("iceblink.device"),
        required_features: optional_features() & adapter.features(),
        required_limits: limits,
        memory_hints: memory_hints(),
        ..Default::default()
    }
}

/// Returns `extensions` and the extensions the native swap chains of `swap_chain` need, where
/// `adapter` has them.
pub(super) fn with_presenter(
    adapter: &wgpu::Adapter,
    extensions: &[&'static CStr],
    swap_chain: SwapChain,
) -> Vec<&'static CStr> {
    let mut extensions = extensions.to_vec();
    if swap_chain.opens_native(adapter) {
        add_native(adapter, &mut extensions);
    }
    extensions
}

/// Adds the extensions native swap chains need that `adapter` has: incremental present, display
/// timing, and when presents are timed, present ids and waits.
#[cfg(vulkan_hal)]
fn add_native(adapter: &wgpu::Adapter, extensions: &mut Vec<&'static CStr>) {
    let granted = [extensions::INCREMENTAL_PRESENT, extensions::DISPLAY_TIMING]
        .into_iter()
        .filter(|name| extensions::granted_by(adapter, &[*name]));
    let timed =
        crate::present::native::times_presents() && extensions::present_timing_granted(adapter);
    let timing = timed
        .then_some(extensions::PRESENT_TIMING)
        .into_iter()
        .flatten();
    for name in granted.chain(timing) {
        if !extensions.contains(&name) {
            extensions.push(name);
        }
    }
}

#[cfg(not(vulkan_hal))]
fn add_native(_adapter: &wgpu::Adapter, _extensions: &mut Vec<&'static CStr>) {}

/// Returns the features a device asks for where the adapter has them.
fn optional_features() -> wgpu::Features {
    let features = wgpu::Features::DUAL_SOURCE_BLENDING | wgpu::Features::PIPELINE_CACHE;
    if crate::gpu::timing::requested() {
        features | wgpu::Features::TIMESTAMP_QUERY
    } else {
        features
    }
}

/// The smallest block of device memory resources are suballocated from.
const MIN_MEMORY_BLOCK: u64 = 8 << 20;

/// The largest block of device memory resources are suballocated from.
const MAX_MEMORY_BLOCK: u64 = 256 << 20;

/// Returns the memory hints a device is requested with.
fn memory_hints() -> wgpu::MemoryHints {
    wgpu::MemoryHints::Manual {
        suballocated_device_memory_block_size: MIN_MEMORY_BLOCK..MAX_MEMORY_BLOCK,
    }
}
