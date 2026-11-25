//! Vector frames built by hand and drawn through a path rasteriser, with the renderer's
//! composites done on the host.

pub mod canvas;
pub mod draw;
pub mod readback;
pub mod scene;

use crate::gpu::TestDevice;

/// Opens a device on the requested driver with a pipeline cache where the adapter has one.
///
/// Returns `None` in the cases [`TestDevice::open`] describes.
pub fn open_device() -> Option<TestDevice> {
    TestDevice::open_with(wgpu::Features::PIPELINE_CACHE)
}
