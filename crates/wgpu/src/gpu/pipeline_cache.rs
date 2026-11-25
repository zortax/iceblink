//! The driver's pipeline cache of the renderer's own pipelines, kept on disk between runs.

use std::path::Path;

pub use crate::pipeline_cache::DIRECTORY_VARIABLE;
pub(crate) use crate::pipeline_cache::PipelineCache;

use crate::gpu::device::Gpu;

// The build script hashes the shader modules and the code that describes the pipelines.
include!(concat!(env!("OUT_DIR"), "/pipelines.rs"));

/// Loads the cache of the renderer's pipelines on `gpu` from `directory`, or from the default
/// directory.
pub(crate) fn load(gpu: &Gpu, directory: Option<&Path>) -> PipelineCache {
    PipelineCache::load(
        gpu.device(),
        &gpu.adapter().get_info(),
        directory,
        "scene",
        PIPELINES,
    )
}
