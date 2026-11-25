//! The rasteriser: outlines flattened on the host, multisampled on the device, and converted to
//! straight colour once per scratch layer.

mod band;
mod collect;
mod contract;
mod outlines;
mod record;
mod stats;
mod tables;

use iceblink::DeviceRect;

use crate::upload::{TableTexture, UploadBelt};
use crate::vector::{Capacity, VectorTarget};
use crate::vector_coverage::geometry::Segment;
use crate::vector_coverage::instance::{Item, Run};
use crate::vector_coverage::pipeline::Pipelines;
use crate::vector_coverage::scratch::Scratch;

use self::outlines::Outlines;

pub use self::stats::Rasterised;

/// A vector rasteriser that needs no compute shaders.
///
/// It draws through the render pipeline alone and reads its tables as textures. What it draws
/// has these limits:
///
/// - Each pixel is decided by sixteen samples, so an edge lands on one of seventeen coverage
///   levels. Interiors are exact.
/// - A gradient is filled with the mean colour of its ramp.
/// - The cost of a path grows with its area times the segments of its outline.
pub struct CoverageRaster {
    /// The device it draws on.
    device: wgpu::Device,
    /// The queue it submits to.
    queue: wgpu::Queue,
    pipelines: Pipelines,
    /// The two textures a pass is drawn through.
    scratch: Scratch,
    /// The segment table: every band's segments, then every clip run's.
    segments: Vec<Segment>,
    /// Per band, where its segments start in `segments` and how many there are.
    bands: Vec<[u32; 4]>,
    /// The outline of the fill being cut into bands.
    flattened: Vec<Segment>,
    /// Outlines kept across frames.
    outlines: Outlines,
    /// Per band of the fill being cut, its count and then where it starts.
    tally: Vec<u32>,
    /// The clip runs.
    runs: Vec<Run>,
    /// The fills, in draw order.
    items: Vec<Item>,
    /// Per pass of the round, its first fill and how many there are.
    spans: Vec<(u32, u32)>,
    /// The pass regions the layering is packed from.
    regions: Vec<DeviceRect>,
    /// Per layer, the passes of the round placed in it.
    layered: Vec<Vec<usize>>,
    /// The targets a clear writes.
    targets: Vec<VectorTarget>,
    /// The device copies of the tables.
    buffers: Buffers,
    /// Mapped staging chunks the tables are written through.
    uploader: UploadBelt,
    /// What the last frame cost and could not do.
    last: Rasterised,
    /// How many layers the last frame's fullest round needed.
    depth: u32,
    /// How many rounds the last frame needed.
    rounds: usize,
    /// How much scratch one round may hold.
    capacity: Capacity,
}

impl std::fmt::Debug for CoverageRaster {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CoverageRaster")
            .field("scratch", &self.scratch.extent())
            .field("layers", &self.scratch.layers())
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}

/// The device copies of the tables the coverage shader reads.
#[derive(Debug)]
struct Buffers {
    /// The fills.
    items: TableTexture,
    /// The segment table.
    segments: TableTexture,
    /// The clip runs.
    runs: TableTexture,
    /// The band table.
    bands: TableTexture,
}

impl CoverageRaster {
    /// Returns a rasteriser on `device` and `queue`, with one scratch layer of `width` by
    /// `height` pixels.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, width: u32, height: u32) -> Self {
        let mut scratch = Scratch::new();
        scratch.ensure(device, width.max(1), height.max(1), Scratch::LAYERS);
        Self {
            pipelines: Pipelines::new(device),
            scratch,
            segments: Vec::new(),
            bands: Vec::new(),
            flattened: Vec::new(),
            outlines: Outlines::default(),
            tally: Vec::new(),
            runs: Vec::new(),
            items: Vec::new(),
            spans: Vec::new(),
            regions: Vec::new(),
            layered: Vec::new(),
            targets: Vec::new(),
            buffers: Buffers {
                items: TableTexture::new(device, "iceblink.vector.coverage.items"),
                segments: TableTexture::new(device, "iceblink.vector.coverage.segments"),
                runs: TableTexture::new(device, "iceblink.vector.coverage.runs"),
                bands: TableTexture::new(device, "iceblink.vector.coverage.bands"),
            },
            uploader: UploadBelt::default(),
            last: Rasterised::default(),
            depth: 0,
            rounds: 0,
            capacity: clamped(Capacity::DEFAULT, device),
            device: device.clone(),
            queue: queue.clone(),
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

    /// Returns what the last frame cost and could not do.
    pub fn last_frame(&self) -> Rasterised {
        self.last
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

    /// Returns how many layers each of the two scratch textures has.
    pub fn layers(&self) -> u32 {
        self.scratch.layers()
    }

    /// Returns the width and height of every scratch layer.
    pub fn extent(&self) -> (u32, u32) {
        self.scratch.extent()
    }

    /// Returns the bytes of device memory both scratch textures hold.
    pub fn scratch_bytes(&self) -> u64 {
        self.scratch.bytes()
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
