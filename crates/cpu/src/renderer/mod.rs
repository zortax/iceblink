//! The CPU renderer: its state and its options.

mod compose;
mod contract;
mod present;
mod shift;

use std::cell::Cell;
use std::sync::Arc;

use iceblink::damage::DamageSet;
use iceblink::{ClientId, PresentMode, RenderTarget, SubpixelOrder, TextureId};

use crate::draw::marks::Bins;
use crate::external::{ExternalImage, Externals};
use crate::frame::Worker;
use crate::frame::instances::Instances;
use crate::frame::resident::Resident;
use crate::math::text::TextGamma;
use crate::pages::Pages;
use crate::present::{CpuPresenter, PixmapPresenter};
use crate::tables::Tables;
use crate::target::Pixmap;
use crate::vector::Vectors;

/// The largest page and target extent the renderer takes, in texels.
const MAX_TEXTURE_SIZE: u32 = 16384;

/// A renderer that draws frames on the processor, into a kept target or into its presenter's
/// images.
///
/// It draws every frame kind, clip and filter, all seventeen blend modes and subpixel sprites, and
/// it keeps resident chunks and the rasters of keyed vector passes. It applies shifts in
/// kept-target mode. Application shaders, which are WGSL programs, cannot run on it, so it reports
/// `custom_shaders: false` and the compositor sends it no shader quads or shader filters.
///
/// Large draw rectangles are split into bands of rows, which worker threads draw in parallel.
pub struct CpuRenderer {
    presenter: Box<dyn CpuPresenter>,
    target: Option<RenderTarget>,
    /// The kept target, which holds the last composed frame in kept-target mode.
    kept: Pixmap,
    /// Whether the kept target holds nothing the next frame may build on.
    full_damage_next: bool,
    /// Whether the last age `target_age` reported was `Kept`.
    promised_kept: Cell<bool>,
    /// The present damage composed since the last present, while a composed frame waits to be
    /// presented.
    unpresented: Option<DamageSet>,
    /// Whether a frame was held back with `Skipped(Held)`.
    held: bool,
    /// Whether a `configure` changed the target since the last frame.
    reconfigured: bool,
    pages: Pages,
    tables: Tables,
    resident: Resident,
    instances: Instances,
    bins: Bins,
    vectors: Vectors,
    externals: Externals,
    workers: Vec<Worker>,
    subpixel_order: SubpixelOrder,
    gamma: TextGamma,
    resident_chunks: bool,
    frames: u64,
    acquire_block: std::time::Duration,
}

impl core::fmt::Debug for CpuRenderer {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CpuRenderer")
            .field("target", &self.target)
            .field("mode", &self.presenter.capabilities().mode)
            .field("workers", &self.workers.len())
            .finish_non_exhaustive()
    }
}

impl CpuRenderer {
    /// Returns a renderer that presents through `presenter`, drawing on one worker per available
    /// processor.
    pub fn new(presenter: Box<dyn CpuPresenter>) -> Self {
        let workers = std::thread::available_parallelism().map_or(1, |count| count.get().min(16));
        Self {
            presenter,
            target: None,
            kept: Pixmap::new(0, 0),
            full_damage_next: true,
            promised_kept: Cell::new(false),
            unpresented: None,
            held: false,
            reconfigured: false,
            pages: Pages::default(),
            tables: Tables::default(),
            resident: Resident::default(),
            instances: Instances::default(),
            bins: Bins::default(),
            vectors: Vectors::default(),
            externals: Externals::default(),
            workers: (0..workers).map(|_| Worker::default()).collect(),
            subpixel_order: SubpixelOrder::default(),
            gamma: TextGamma::display(),
            resident_chunks: true,
            frames: 0,
            acquire_block: std::time::Duration::ZERO,
        }
    }

    /// Returns a renderer of a `size` target that presents into one kept pixmap.
    pub fn offscreen(size: [u32; 2]) -> Self {
        Self::new(Box::new(PixmapPresenter::kept(size)))
    }

    /// Returns the renderer drawing on `threads` worker threads, at least one.
    #[must_use]
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.workers = (0..threads.max(1)).map(|_| Worker::default()).collect();
        self
    }

    /// Sets whether the renderer reports `resident_chunks`, so the compositor sends it chunk
    /// notes.
    pub fn set_resident_chunks(&mut self, resident: bool) {
        self.resident_chunks = resident;
    }

    /// Sets whether the renderer keeps the rasters of keyed vector passes and reports
    /// `kept_rasters`.
    pub fn set_rasters_kept(&mut self, kept: bool) {
        self.vectors.keeping = kept;
        if !kept {
            self.vectors.release();
        }
    }

    /// Binds `image` to the external texture slot `id` of `client`.
    ///
    /// External quads of the slot draw the image from the next frame on.
    pub fn bind_external(&mut self, client: ClientId, id: TextureId, image: Arc<ExternalImage>) {
        self.externals.bind(client, id, image);
    }

    /// Removes the image bound to the external texture slot `id` of `client`.
    ///
    /// External quads of the slot draw nothing from the next frame on.
    pub fn unbind_external(&mut self, client: ClientId, id: TextureId) {
        self.externals.unbind(client, id);
    }

    /// Returns the kept target, which holds the last composed frame in kept-target mode.
    pub fn composed(&self) -> &Pixmap {
        &self.kept
    }

    fn buffer_age(&self) -> bool {
        self.presenter.capabilities().mode == PresentMode::BufferAge
    }
}
