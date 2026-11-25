//! A path renderer on one device, shared by the rasterisers made from it.

mod cache;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use iceblink_wgpu::vector::VectorError;
use vello::{AaSupport, Renderer, RendererOptions};

pub use self::cache::DIRECTORY_VARIABLE;
use crate::raster::VelloOptions;

/// A vello path renderer on one device, with the fixed device memory it holds.
///
/// Rasterisers made from it with [`VelloRaster::sharing`](crate::VelloRaster::sharing) share it,
/// and rasterisations through it run one at a time.
// The renderer's fixed buffers take hundreds of megabytes, so one per window would spend a
// device's budget on copies.
pub struct SharedRenderer {
    /// The device the renderer was built on.
    device: wgpu::Device,
    renderer: Mutex<Renderer>,
    /// The device memory the renderer holds whatever it draws, in bytes.
    // Measured with the device's allocator across construction and across the first
    // rasterisation, where the large buffers are allocated.
    fixed: AtomicU64,
    /// Whether the first rasterisation has been measured.
    settled: AtomicBool,
}

impl SharedRenderer {
    /// Builds a renderer on `device`, with the pipeline cache that `options` choose.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Device`] when the path renderer cannot be built on the device.
    pub fn new(device: &wgpu::Device, options: &VelloOptions) -> Result<Arc<Self>, VectorError> {
        Self::build(device, options)
            .map(Arc::new)
            .map_err(|error| VectorError::Device {
                detail: error.to_string(),
            })
    }

    fn build(device: &wgpu::Device, options: &VelloOptions) -> Result<Self, vello::Error> {
        let cache = cache::load(
            device,
            options.adapter.as_ref(),
            options.cache_directory.as_deref(),
        );

        let before = allocated(device);
        let renderer = Renderer::new(
            device,
            RendererOptions {
                use_cpu: false,
                // Area coverage alone. Each variant compiles the largest shader again, and the
                // multisampled ones showed no difference worth that on overlapping strokes,
                // rounded icons and self-intersecting paths: at worst 48 levels over 845 edge
                // pixels.
                antialiasing_support: AaSupport {
                    area: true,
                    msaa8: false,
                    msaa16: false,
                },
                num_init_threads: None,
                pipeline_cache: cache.handle().cloned(),
            },
        )?;
        cache.store();
        let fixed = allocated(device).saturating_sub(before);

        Ok(Self {
            device: device.clone(),
            renderer: Mutex::new(renderer),
            fixed: AtomicU64::new(fixed),
            settled: AtomicBool::new(false),
        })
    }

    /// Returns the device the renderer was built on.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Runs `rasterise`, and on the first call adds what the device's allocator reports across it
    /// to the fixed memory.
    pub(crate) fn measuring<T>(&self, rasterise: impl FnOnce() -> T) -> T {
        if self.settled.load(Ordering::Relaxed) {
            return rasterise();
        }

        let before = allocated(&self.device);
        let value = rasterise();
        let after = allocated(&self.device);
        self.fixed
            .fetch_add(after.saturating_sub(before), Ordering::Relaxed);
        self.settled.store(true, Ordering::Relaxed);
        value
    }

    /// Locks the renderer for one rasterisation.
    ///
    /// # Panics
    ///
    /// Panics if an earlier holder panicked while rasterising, which leaves the renderer's
    /// buffers in an unknown state.
    pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, Renderer> {
        self.renderer.lock().expect("the path renderer is usable")
    }

    /// Returns the device memory the renderer holds whatever it draws, in bytes.
    ///
    /// The figure is final once a rasterisation has run.
    pub fn fixed_bytes(&self) -> u64 {
        self.fixed.load(Ordering::Relaxed)
    }
}

impl std::fmt::Debug for SharedRenderer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SharedRenderer")
            .field("fixed", &self.fixed_bytes())
            .finish_non_exhaustive()
    }
}

/// Returns the bytes the device's allocator has handed out, or zero where it keeps no report.
fn allocated(device: &wgpu::Device) -> u64 {
    device
        .generate_allocator_report()
        .map_or(0, |report| report.total_allocated_bytes)
}
