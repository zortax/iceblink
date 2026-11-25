//! Opening a Vulkan device with device extensions wgpu does not request by itself.

// Reaching the Vulkan device through wgpu-hal needs `unsafe`, with each block's invariants stated.
#![allow(unsafe_code)]

#[cfg(not(vulkan_hal))]
mod absent;
#[cfg(vulkan_hal)]
mod vulkan;

use std::ffi::CStr;

/// The extension that lets a present name the rectangles that changed.
#[cfg(vulkan_hal)]
pub(crate) const INCREMENTAL_PRESENT: &CStr = c"VK_KHR_incremental_present";

/// The extension that tells a swap chain the display's refresh interval.
#[cfg(vulkan_hal)]
pub(crate) const DISPLAY_TIMING: &CStr = c"VK_GOOGLE_display_timing";

/// The extensions that let a swap chain wait until a present is shown on the display.
#[cfg(vulkan_hal)]
pub(crate) const PRESENT_TIMING: [&CStr; 2] = [c"VK_KHR_present_id", c"VK_KHR_present_wait"];

#[cfg(vulkan_hal)]
pub(crate) use self::vulkan::{granted_by, open, present_timing_granted};

#[cfg(not(vulkan_hal))]
pub(crate) use self::absent::{granted_by, open, present_timing_granted};

/// A device opened with Vulkan extensions: the device, its queue and the extensions it has.
pub(crate) type Opened = (wgpu::Device, wgpu::Queue, Vec<&'static CStr>);
