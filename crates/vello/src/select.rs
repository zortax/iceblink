//! Which rasteriser a device gets.

use iceblink_wgpu::vector::VectorRaster;
use iceblink_wgpu::vector_coverage::CoverageRaster;

use crate::raster::{VelloOptions, VelloRaster};

/// The rasteriser a device runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// The vello compute rasteriser.
    Compute,
    /// The coverage rasteriser, for a device that cannot run vello.
    Coverage,
}

/// Returns the rasteriser `adapter` can run.
///
/// Vello needs compute shaders, the half-float packing its shaders use, and `Rgba8Unorm` storage
/// textures. Every other adapter gets the coverage rasteriser.
// The path renderer panics on a shader it cannot compile instead of returning an error, so the
// check comes before it is built. Software Vulkan drivers lack half floats where they lack the
// native type.
pub fn chosen(adapter: &wgpu::Adapter) -> Choice {
    let downlevel = adapter.get_downlevel_capabilities().flags;
    let storage = adapter
        .get_texture_format_features(wgpu::TextureFormat::Rgba8Unorm)
        .allowed_usages
        .contains(wgpu::TextureUsages::STORAGE_BINDING);
    if downlevel.contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
        && downlevel.contains(wgpu::DownlevelFlags::SHADER_F16_IN_F32)
        && storage
    {
        Choice::Compute
    } else {
        Choice::Coverage
    }
}

/// Returns the rasteriser for a device opened on `adapter`, for an output of `size` device
/// pixels.
///
/// The rasteriser is the one [`chosen`] returns. When vello cannot be built on a device that should
/// run it, the coverage rasteriser is returned instead, and the failure is logged. Without an
/// adapter in `options`, vello's pipeline cache is keyed by `adapter`.
pub fn for_device(
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    size: [u32; 2],
    options: &VelloOptions,
) -> Box<dyn VectorRaster> {
    let [width, height] = size.map(|side| side.max(1));
    let info = adapter.get_info();
    let name = info.name.clone();
    if chosen(adapter) == Choice::Compute {
        let options = VelloOptions {
            adapter: options.adapter.clone().or(Some(info)),
            cache_directory: options.cache_directory.clone(),
        };
        match VelloRaster::new(device, queue, &options) {
            Ok(raster) => return Box::new(raster),
            Err(failure) => tracing::warn!(
                %failure,
                adapter = %name,
                "the vello path renderer did not build on this device, falling back"
            ),
        }
    } else {
        tracing::info!(
            adapter = %name,
            "this device cannot run vello, so paths are drawn by the coverage rasteriser"
        );
    }
    Box::new(CoverageRaster::new(device, queue, width, height))
}
