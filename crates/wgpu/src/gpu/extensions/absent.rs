//! Device extensions where wgpu has no Vulkan backend.

use std::ffi::CStr;

use crate::gpu::extensions::Opened;

pub(crate) fn granted_by(_adapter: &wgpu::Adapter, extensions: &[&'static CStr]) -> bool {
    extensions.is_empty()
}

pub(crate) fn present_timing_granted(_adapter: &wgpu::Adapter) -> bool {
    false
}

pub(crate) fn open(
    _adapter: &wgpu::Adapter,
    _extensions: &[&'static CStr],
    _descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Option<Opened> {
    None
}
