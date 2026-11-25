//! The two rasterisers on one device.

use iceblink_vello::{VelloOptions, VelloRaster};
use iceblink_wgpu::vector::{Capacity, VectorRaster};
use iceblink_wgpu::vector_coverage::CoverageRaster;

use iceblink_testing::gpu::TestDevice;

/// Which rasteriser a test draws through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Which {
    Vello,
    Coverage,
}

/// Returns a vello rasteriser on `gpu`.
///
/// # Panics
///
/// Panics when vello cannot be built on the device.
pub(crate) fn vello(gpu: &TestDevice) -> VelloRaster {
    VelloRaster::new(&gpu.device, &gpu.queue, &VelloOptions::default())
        .expect("vello builds on the test device")
}

/// Returns a coverage rasteriser on `gpu` with one scratch layer of `side` square.
pub(crate) fn coverage(gpu: &TestDevice, side: i32) -> CoverageRaster {
    CoverageRaster::new(&gpu.device, &gpu.queue, side as u32, side as u32)
}

/// Returns a rasteriser of `which` kind on `gpu`, holding `capacity` per round.
pub(crate) fn raster(
    gpu: &TestDevice,
    which: Which,
    side: i32,
    capacity: Capacity,
) -> Box<dyn VectorRaster> {
    match which {
        Which::Vello => Box::new(vello(gpu).with_capacity(capacity)),
        Which::Coverage => Box::new(coverage(gpu, side).with_capacity(capacity)),
    }
}
