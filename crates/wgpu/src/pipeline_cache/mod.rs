//! Compiled pipelines kept between runs.
//!
//! A [`PipelineCache`] keeps the driver's compiled pipelines in a file of its own. The file is
//! named by the pipeline set it holds and by the adapter and driver that compiled it, so builds with
//! other shaders and other drivers keep files of their own. A directory keeps the four files of one
//! adapter that were used most recently.
//!
//! A file is replaced only by a cache that holds more, so processes that run at the same time never
//! shrink it. A file the driver refuses is replaced by the next write.

mod file;
mod fingerprint;

use std::cell::Cell;
use std::path::Path;

pub use self::file::{DIRECTORY_VARIABLE, default_directory};
pub use self::fingerprint::fingerprint;

use self::file::CacheFile;

/// A driver pipeline cache, and the file its contents are kept in.
///
/// It is empty on a device without pipeline caches or without a directory to keep it in.
#[derive(Debug, Default)]
pub struct PipelineCache {
    cache: Option<wgpu::PipelineCache>,
    file: Option<CacheFile>,
    /// Whether the cache was lent out since the last write.
    unsaved: Cell<bool>,
    /// Whether the cache started from a file the driver accepted.
    loaded: bool,
}

impl PipelineCache {
    /// Loads the cache of pipeline set `name` with fingerprint `pipelines`, for `device` on the
    /// adapter `adapter` describes.
    ///
    /// Without `directory`, the cache is kept in [`default_directory`].
    pub fn load(
        device: &wgpu::Device,
        adapter: &wgpu::AdapterInfo,
        directory: Option<&Path>,
        name: &str,
        pipelines: u64,
    ) -> Self {
        if !device.features().contains(wgpu::Features::PIPELINE_CACHE) {
            return Self::default();
        }
        let Some(directory) = directory.map(Path::to_path_buf).or_else(default_directory) else {
            return Self::default();
        };
        let file = CacheFile::new(directory, name, adapter, pipelines);
        let data = file.read();
        let cache = create(device, data.as_deref());
        // A refused file comes back as an empty cache, which the next write replaces.
        let refused = data.as_ref().is_some_and(|data| {
            cache
                .get_data()
                .is_none_or(|kept| 2 * kept.len() < data.len())
        });
        file.loaded(refused);
        Self {
            cache: Some(cache),
            file: Some(file),
            unsaved: Cell::new(false),
            loaded: data.is_some() && !refused,
        }
    }

    /// Reports whether the cache started from a file the driver accepted.
    pub fn loaded(&self) -> bool {
        self.loaded
    }

    /// Returns the cache to build pipelines with, if the device has pipeline caches.
    pub fn handle(&self) -> Option<&wgpu::PipelineCache> {
        if self.cache.is_some() {
            self.unsaved.set(true);
        }
        self.cache.as_ref()
    }

    /// Writes the cache to its file if it was lent out since the last write and holds more than
    /// the file.
    ///
    /// A failed write is logged.
    // A lost write costs a later launch a compilation and nothing else.
    pub fn store(&self) {
        let (Some(cache), Some(file)) = (&self.cache, &self.file) else {
            return;
        };
        if !self.unsaved.replace(false) {
            return;
        }
        if let Some(data) = cache.get_data() {
            file.write(&data);
        }
    }
}

/// Creates the cache object from the stored `data`.
#[expect(
    unsafe_code,
    reason = "creating a pipeline cache from stored data has no safe form"
)]
fn create(device: &wgpu::Device, data: Option<&[u8]>) -> wgpu::PipelineCache {
    // SAFETY: the data is absent or was written by `store` into a file named by this adapter and
    // driver, wgpu validates its header before the driver reads it, and `fallback` turns data the
    // driver rejects into an empty cache.
    unsafe {
        device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
            label: Some("iceblink.pipeline_cache"),
            data,
            fallback: true,
        })
    }
}
