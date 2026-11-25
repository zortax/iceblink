//! The compiled pipelines of the path renderer, kept between runs.

use std::path::Path;

pub use iceblink_wgpu::pipeline_cache::DIRECTORY_VARIABLE;
pub(crate) use iceblink_wgpu::pipeline_cache::PipelineCache;

/// Loads the cache of the path renderer's pipelines on `device` from `directory`, or from the
/// default directory.
///
/// Without `adapter`, the cache is empty.
pub(crate) fn load(
    device: &wgpu::Device,
    adapter: Option<&wgpu::AdapterInfo>,
    directory: Option<&Path>,
) -> PipelineCache {
    let Some(adapter) = adapter else {
        return PipelineCache::default();
    };
    // vello's own version is not reachable here. The renderer's options and this crate's version
    // stand for its pipeline set.
    let pipelines = iceblink_wgpu::pipeline_cache::fingerprint(&[
        b"vello area coverage",
        env!("CARGO_PKG_VERSION").as_bytes(),
    ]);
    PipelineCache::load(device, adapter, directory, "vector", pipelines)
}
