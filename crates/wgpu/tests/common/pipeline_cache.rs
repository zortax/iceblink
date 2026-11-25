//! Devices and cache directories for the pipeline cache tests.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use iceblink_testing::gpu::{TestDevice, skip};
use iceblink_wgpu::pipeline_cache::PipelineCache;

/// Returns a device with pipeline caches, or `None` after reporting the skip.
pub(crate) fn device() -> Option<TestDevice> {
    let device = TestDevice::open_with(wgpu::Features::PIPELINE_CACHE)?;
    if !device
        .device
        .features()
        .contains(wgpu::Features::PIPELINE_CACHE)
    {
        return skip("the adapter has no pipeline caches");
    }
    Some(device)
}

/// Returns an empty directory of its own under the system's temporary directory.
pub(crate) fn directory() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let directory = std::env::temp_dir().join(format!(
        "iceblink-pipeline-cache-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("the test directory is made");
    directory
}

/// Loads the cache of pipeline set "test" with fingerprint `pipelines` from `directory`.
pub(crate) fn load(device: &TestDevice, directory: &Path, pipelines: u64) -> PipelineCache {
    PipelineCache::load(
        &device.device,
        &device.adapter.get_info(),
        Some(directory),
        "test",
        pipelines,
    )
}

/// Builds `count` compute pipelines through `cache`, numbered from `first`.
///
/// The shaders differ by number and by process, so tests that number their pipelines apart
/// build pipelines no earlier test or run compiled.
pub(crate) fn build(device: &TestDevice, cache: &PipelineCache, first: u32, count: u32) {
    // A pipeline the driver compiled before is not stored again.
    let run = std::process::id();
    for index in first..first + count {
        let source = format!(
            "@group(0) @binding(0) var<storage, read_write> out: array<u32>;\n\
             @compute @workgroup_size(1) fn main() {{ out[0] = {index}u; out[1] = {run}u; }}\n"
        );
        let module = device
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
        let _ = device
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: None,
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: cache.handle(),
            });
    }
}

/// Returns the names and lengths of the files in `directory`, sorted by name.
pub(crate) fn files(directory: &Path) -> Vec<(String, u64)> {
    let mut files: Vec<(String, u64)> = std::fs::read_dir(directory)
        .expect("the test directory reads")
        .filter_map(Result::ok)
        .map(|entry| {
            let length = entry.metadata().map_or(0, |metadata| metadata.len());
            (entry.file_name().to_string_lossy().into_owned(), length)
        })
        .collect();
    files.sort();
    files
}
