//! The vello rasteriser, which encodes the passes of each scratch layer into one scene and
//! rasterises it once.

mod cached;
mod contract;
mod encode;
mod paint;
mod prepare;
mod scratch;

use std::path::PathBuf;
use std::sync::Arc;

use iceblink::DeviceRect;
use iceblink_wgpu::vector::{Capacity, VectorError, VectorTarget};

use self::cached::Encodings;
pub use self::encode::Encoded;
use self::scratch::Scratch;
use crate::device::SharedRenderer;

/// The settings a [`VelloRaster`] is built with.
#[derive(Clone, Debug, Default)]
pub struct VelloOptions {
    /// The adapter the device was opened on, which keys the compiled-pipeline cache.
    ///
    /// Without it no pipelines are kept between runs.
    pub adapter: Option<wgpu::AdapterInfo>,
    /// The directory compiled pipelines are kept in.
    ///
    /// Without it the directory in `ICEBLINK_CACHE_DIR` is used, then
    /// `iceblink/pipelines` under `XDG_CACHE_HOME` or under `~/.cache`.
    pub cache_directory: Option<PathBuf>,
}

/// A vector rasteriser on the vello compute path renderer.
///
/// Every scratch layer a round uses is rasterised once, with all of its passes in one scene.
pub struct VelloRaster {
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// The path renderer it draws through.
    shared: Arc<SharedRenderer>,
    /// The texture passes are rasterised into.
    scratch: Scratch,
    /// One reusable scene per layer.
    scenes: Vec<vello::Scene>,
    /// The pass regions the layering is packed from.
    regions: Vec<DeviceRect>,
    /// Per layer, the passes of the round placed in it.
    layered: Vec<Vec<usize>>,
    /// The targets a clear writes.
    targets: Vec<VectorTarget>,
    /// Scratch space for gradient stops.
    stops: Vec<(f32, [f32; 4])>,
    /// Encoded paths kept across frames.
    encodings: Encodings,
    /// What the last frame's encoding cost and could not do.
    last: Encoded,
    /// How many passes the last frame rasterised.
    passes: u32,
    /// How many times the path renderer ran, since the rasteriser was made.
    renders: u64,
    /// How many layers the last frame's fullest round needed.
    depth: u32,
    /// How many rounds the last frame needed.
    rounds: usize,
    /// How much scratch one round may hold.
    capacity: Capacity,
}

impl VelloRaster {
    /// Returns a rasteriser on `device` and `queue` with a path renderer of its own.
    ///
    /// The scratch is allocated by the first frame with passes.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Device`] when the path renderer cannot be built on the device.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        options: &VelloOptions,
    ) -> Result<Self, VectorError> {
        Ok(Self::sharing(SharedRenderer::new(device, options)?, queue))
    }

    /// Returns a rasteriser that draws through `shared` on its device, submitting to `queue`.
    ///
    /// `queue` is the queue of the device `shared` was built on. The scratch is allocated by the
    /// first frame with passes.
    pub fn sharing(shared: Arc<SharedRenderer>, queue: &wgpu::Queue) -> Self {
        let device = shared.device().clone();
        Self {
            capacity: clamped(Capacity::DEFAULT, &device),
            device,
            queue: queue.clone(),
            shared,
            scratch: Scratch::new(),
            scenes: (0..Scratch::MIN_LAYERS)
                .map(|_| vello::Scene::new())
                .collect(),
            regions: Vec::new(),
            layered: Vec::new(),
            targets: Vec::new(),
            stops: Vec::new(),
            encodings: Encodings::with_cap(Encodings::BYTE_CAP),
            last: Encoded::default(),
            passes: 0,
            renders: 0,
            depth: 0,
            rounds: 0,
        }
    }

    /// Returns this rasteriser with `capacity` of scratch per round.
    ///
    /// The layers are clamped to between 1 and 64, and the side to the device's largest texture.
    #[must_use]
    pub fn with_capacity(self, capacity: Capacity) -> Self {
        Self {
            capacity: clamped(capacity, &self.device),
            ..self
        }
    }

    /// Returns the path renderer this rasteriser shares.
    pub fn shared(&self) -> &Arc<SharedRenderer> {
        &self.shared
    }

    /// Returns what the last frame's encoding cost and could not do.
    ///
    /// Every round the frame prepared counts, including a round prepared again.
    pub fn last_frame(&self) -> Encoded {
        self.last
    }

    /// Returns how many passes the last frame rasterised, counting every preparation.
    pub fn passes(&self) -> u32 {
        self.passes
    }

    /// Returns how many times the path renderer ran, since the rasteriser was made.
    pub fn renders(&self) -> u64 {
        self.renders
    }

    /// Returns how many scratch layers the last frame's fullest round needed.
    ///
    /// The allocated layers, [`layers`](Self::layers), follow this demand down after a delay.
    pub fn depth(&self) -> u32 {
        self.depth
    }

    /// Returns how many rounds the last frame needed.
    pub fn rounds(&self) -> usize {
        self.rounds
    }

    /// Returns how many encodings are held, and how many lookups found an encoding and how many
    /// encoded since the rasteriser was made.
    pub fn cache(&self) -> (usize, (u64, u64)) {
        (self.encodings.len(), self.encodings.counts())
    }

    /// Returns how many scratch layers are allocated.
    pub fn layers(&self) -> u32 {
        self.scratch.layers()
    }

    /// Returns the width and height of every scratch layer.
    pub fn extent(&self) -> (u32, u32) {
        self.scratch.extent()
    }
}

impl std::fmt::Debug for VelloRaster {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VelloRaster")
            .field("scratch", &self.scratch.extent())
            .field("encodings", &self.encodings)
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}

/// Returns `capacity` with its layers clamped to the scratch's range and its side to the largest
/// texture of `device`.
fn clamped(capacity: Capacity, device: &wgpu::Device) -> Capacity {
    let largest = device.limits().max_texture_dimension_2d;
    Capacity {
        side: capacity
            .side
            .min(i32::try_from(largest).unwrap_or(i32::MAX)),
        layers: capacity.layers.clamp(1, Scratch::MAX_LAYERS),
    }
}
