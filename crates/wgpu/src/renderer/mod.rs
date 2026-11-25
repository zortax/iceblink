//! The renderer: the device, the kept target, and everything a frame passes through.
//!
//! A frame's resource work, chunk notes and table deltas are applied first, whatever happens to
//! the frame afterwards. In kept-target mode the frame is then composed into the kept target and
//! copied into an acquired image, covering only the image's debt. In buffer-age mode it is
//! composed straight into the image the presenter acquired before the frame was built.

pub(crate) mod buffers;
mod builder;
mod compose;
mod contract;
mod debt;
mod externals;
mod latency;
mod outcome;
mod present;
pub(crate) mod readback;
mod record;
mod recovery;
mod shared;
mod vectors;

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use iceblink::damage::DamageSet;
use iceblink::geom::{Device, Rect, Size};
use iceblink::{ClientId, PresentMode, RenderTarget, SubpixelOrder, TextureId, VectorBackend};

use crate::effect::EffectSlots;
use crate::frame::shift::ShiftScratch;
use crate::gpu::device::Gpu;
use crate::gpu::formats::Formats;
use crate::gpu::timing::{GpuTimer, GpuTiming};
use crate::pages::Pages;
use crate::pipeline::Pipelines;
use crate::present::WgpuPresenter;
use crate::renderer::buffers::FrameBuffers;
use crate::renderer::externals::Externals;
use crate::renderer::latency::Latency;
use crate::renderer::recovery::Recovery;
use crate::renderer::shared::DeviceState;
use crate::target::group_pool::{GroupPool, GroupPrecision};
use crate::target::marks::MarksScratch;
use crate::target::scene_texture::SceneTexture;
use crate::vector::{Keeping, VectorRaster};

pub use self::builder::Builder;
pub use self::latency::LatencyMarks;
pub use self::readback::Pixels;
pub use self::recovery::ReplacementGpu;
pub use self::shared::SharedGraphics;

/// A function that builds a path rasteriser for the device and the output's size.
///
/// The closure captures the rasteriser's options, such as the directory compiled pipelines are
/// kept in.
pub type VectorFactory = Box<dyn Fn(&Arc<Gpu>, [u32; 2]) -> Box<dyn VectorRaster>>;

/// A function called after a frame's work is submitted and before its image is presented, as a
/// platform's pre-present notification.
pub type PrePresent = Box<dyn Fn(RenderTarget) + Send + Sync>;

/// A function that reports whether the platform takes another image now.
///
/// A frame composed while it does not is held, and
/// [`iceblink::Renderer::present_held`] presents it later.
pub type PresentGate = Box<dyn Fn() -> bool + Send + Sync>;

/// A renderer that draws frames with wgpu and presents them through its presenter.
pub struct WgpuRenderer {
    gpu: Arc<Gpu>,
    presenter: Box<dyn WgpuPresenter>,
    kept: SceneTexture,
    kept_binding: wgpu::BindGroup,
    pipelines: Rc<RefCell<Pipelines>>,
    shared: Option<SharedGraphics>,
    recovery: Recovery,
    buffers: FrameBuffers,
    pages: Pages,
    effects: EffectSlots,
    groups: GroupPool,
    sampler: wgpu::Sampler,
    vectors: Option<Box<dyn VectorRaster>>,
    vector_factory: Option<VectorFactory>,
    /// Whether rasterisers set from now on keep the rasters of keyed passes.
    rasters_kept: bool,
    vector_backend: Option<VectorBackend>,
    vector_name: &'static str,
    vector_shortfall_is_fatal: bool,
    /// Whether a frame had vector passes and no rasteriser to draw them, which is logged once.
    vectors_missed: bool,
    full_damage_next: bool,
    /// Whether the last age `target_age` reported was `Kept`, so the next frame may be culled to
    /// its damage.
    promised_kept: Cell<bool>,
    shift_scratch: Option<ShiftScratch>,
    marks_scratch: MarksScratch,
    /// The damage composed since the last present that was shown on the display, if any.
    unpresented: Option<DamageSet>,
    composed_rects: Vec<Rect<i32, Device>>,
    composed_rows: Vec<Range<u32>>,
    presented: Option<u32>,
    presented_damage: DamageSet,
    target: Option<RenderTarget>,
    subpixel_order: SubpixelOrder,
    externals: Externals,
    pre_present: Option<PrePresent>,
    present_gate: Option<PresentGate>,
    held: bool,
    /// The refreshes the newest frame was planned on, as one of them and the interval.
    refreshes: (u64, u64),
    /// Whether a configure changed the target since the last composed frame.
    reconfigured: bool,
    acquire_block: Duration,
    backends: wgpu::Backends,
    timer: Option<GpuTimer>,
    latency: Latency,
    counters: Counters,
    staging: readback::Staging,
}

/// The renderer's counters, reported through `extra_stats`.
#[derive(Clone, Copy, Debug, Default)]
struct Counters {
    frames: u64,
    vector_frames_dropped: u64,
    deferred: u64,
    unisolated: u64,
    unsupported_blends: u64,
    table_slots_prepared: u64,
    upload_allocations: u64,
}

impl std::fmt::Debug for WgpuRenderer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WgpuRenderer")
            .field("adapter", &self.gpu.describe())
            .field("target", &self.target)
            .field("formats", &self.presenter.formats())
            .finish_non_exhaustive()
    }
}

impl WgpuRenderer {
    /// Assembles a renderer that draws on `device` and presents through `presenter`.
    pub(crate) fn assemble(
        device: Rc<DeviceState>,
        shared: Option<SharedGraphics>,
        presenter: Box<dyn WgpuPresenter>,
        options: builder::Options,
        backends: wgpu::Backends,
    ) -> Self {
        let gpu = Arc::clone(&device.gpu);
        let pipelines = Rc::clone(&device.pipelines);
        let formats = presenter.formats();
        let [width, height] = presenter.extent();
        let size = Size::new(width as i32, height as i32);

        let kept = SceneTexture::new(&gpu, kept_size(presenter.as_ref(), size), formats.scene);
        let kept_binding = bind_kept(&gpu, &pipelines.borrow(), &kept);
        let pages = Pages::new(&gpu, pipelines.borrow().layouts().sampled.clone());
        let mut groups = GroupPool::scaled(size);
        groups.set_precision(
            options
                .precision
                .or_else(GroupPrecision::from_environment)
                .unwrap_or_default(),
        );
        let sampler = filtering_sampler(&gpu);

        // Building the pipelines now keeps the driver's first compilation out of a frame.
        device.prepare(formats);

        Self {
            buffers: FrameBuffers::new(&gpu),
            timer: GpuTimer::new(&gpu),
            gpu,
            presenter,
            kept,
            kept_binding,
            pipelines,
            shared,
            recovery: options.recovery,
            pages,
            effects: EffectSlots::default(),
            groups,
            sampler,
            vectors: None,
            vector_factory: None,
            rasters_kept: true,
            vector_backend: None,
            vector_name: "",
            // A shortfall is fatal where a developer sees it.
            vector_shortfall_is_fatal: cfg!(debug_assertions),
            vectors_missed: false,
            full_damage_next: true,
            promised_kept: Cell::new(false),
            shift_scratch: None,
            marks_scratch: MarksScratch::default(),
            unpresented: None,
            composed_rects: Vec::new(),
            composed_rows: Vec::new(),
            presented: None,
            presented_damage: DamageSet::new(),
            target: None,
            subpixel_order: SubpixelOrder::default(),
            externals: Externals::default(),
            pre_present: options.pre_present,
            present_gate: None,
            held: false,
            refreshes: (0, 0),
            reconfigured: false,
            acquire_block: Duration::ZERO,
            backends,
            latency: options.latency,
            counters: Counters::default(),
            staging: readback::Staging::new(),
        }
    }

    /// Returns the device the renderer draws on.
    pub fn gpu(&self) -> &Arc<Gpu> {
        &self.gpu
    }

    /// Returns the renderer's presenter.
    pub fn wgpu_presenter(&mut self) -> &mut dyn WgpuPresenter {
        self.presenter.as_mut()
    }

    /// Returns the formats the presenter's images use.
    pub fn formats(&self) -> Formats {
        self.presenter.formats()
    }

    /// Sets the gate a composed frame waits on before it is presented.
    pub fn set_present_gate(&mut self, gate: PresentGate) {
        self.present_gate = Some(gate);
    }

    /// Sets whether a path rasteriser running out of capacity panics.
    ///
    /// It panics in debug builds by default, and otherwise draws the passes that fit.
    pub fn set_vector_shortfall_fatal(&mut self, fatal: bool) {
        self.vector_shortfall_is_fatal = fatal;
    }

    /// Sets the path rasteriser vector passes are drawn with.
    pub fn set_vector_raster(&mut self, raster: Box<dyn VectorRaster>, name: &'static str) {
        self.vector_backend = Some(raster.backend());
        self.vector_name = name;
        self.vectors = Some(self.keeping(raster));
        self.vector_factory = None;
    }

    /// Sets whether the rasters of keyed vector passes are kept across frames.
    ///
    /// They are kept by default. The setting applies to rasterisers set afterwards.
    pub fn set_rasters_kept(&mut self, kept: bool) {
        self.rasters_kept = kept;
    }

    /// Returns `raster`, wrapped in a [`Keeping`] when rasters are kept.
    pub(crate) fn keeping(&self, raster: Box<dyn VectorRaster>) -> Box<dyn VectorRaster> {
        if self.rasters_kept {
            Box::new(Keeping::new(raster))
        } else {
            raster
        }
    }

    /// Sets a path rasteriser built once the renderer has a target, and again after the device is
    /// rebuilt.
    pub fn set_vector_factory(
        &mut self,
        factory: VectorFactory,
        backend: VectorBackend,
        name: &'static str,
    ) {
        self.vector_factory = Some(factory);
        self.vectors = None;
        self.vector_backend = Some(backend);
        self.vector_name = name;
        if let Some(target) = self.target {
            self.build_vectors(target.size);
        }
    }

    /// Binds `texture` to a client's external texture, which external quads that refer to it
    /// draw.
    ///
    /// `premultiplied` says whether the texels are premultiplied. A texture bound again replaces
    /// the binding.
    pub fn bind_external(
        &mut self,
        client: ClientId,
        id: TextureId,
        texture: &wgpu::Texture,
        premultiplied: bool,
    ) {
        self.externals.bind(client, id, texture, premultiplied);
    }

    /// Removes a client's external texture binding.
    ///
    /// External quads that refer to it draw nothing.
    pub fn unbind_external(&mut self, client: ClientId, id: TextureId) {
        self.externals.unbind(client, id);
    }

    /// Returns the group target pool.
    pub fn groups(&self) -> &GroupPool {
        &self.groups
    }

    /// Returns the precision group targets are held at.
    pub fn group_precision(&self) -> GroupPrecision {
        self.groups.precision()
    }

    /// Sets the precision group targets are held at.
    ///
    /// A change redraws the next frame whole.
    pub fn set_group_precision(&mut self, precision: GroupPrecision) {
        if precision != self.groups.precision() {
            self.groups.set_precision(precision);
            self.full_damage_next = true;
        }
    }

    /// Sets the memory budget of the group target pool, in bytes.
    pub fn set_group_budget(&mut self, bytes: u64) {
        let precision = self.groups.precision();
        self.groups = GroupPool::new(self.kept.used().size, bytes);
        self.groups.set_precision(precision);
    }

    /// Moves every group target's origin by `margin` device pixels.
    ///
    /// The targets held are dropped, so the next frame allocates them at the new origin.
    pub fn set_target_margin(&mut self, margin: i32) {
        self.groups.set_poison_margin(margin);
    }

    /// Reports whether GPU timing is on for this renderer.
    pub fn gpu_timing_enabled(&self) -> bool {
        self.timer.is_some()
    }

    /// Returns the GPU timing of the latest frame that finished, if timing is on.
    pub fn gpu_timing(&self) -> Option<GpuTiming> {
        self.timer.as_ref().and_then(GpuTimer::latest)
    }

    /// Moves out the GPU timings of frames that finished since the last call.
    pub fn take_gpu_timings(&mut self) -> Vec<GpuTiming> {
        match &mut self.timer {
            Some(timer) => timer.take(&self.gpu),
            None => Vec::new(),
        }
    }

    /// Returns how many frames were timed, if timing is on.
    pub fn gpu_timed_frames(&self) -> Option<u64> {
        self.timer.as_ref().map(GpuTimer::frames)
    }

    /// Returns the rows of the kept target the last frame wrote.
    pub fn composed_rows(&self) -> &[Range<u32>] {
        &self.composed_rows
    }

    /// Returns the rectangles of the kept target the last frame wrote.
    pub fn composed_rects(&self) -> &[Rect<i32, Device>] {
        &self.composed_rects
    }

    /// Returns the damage of the last present.
    pub fn presented_damage(&self) -> &DamageSet {
        &self.presented_damage
    }

    /// Reports whether frames are composed straight into the presenter's images.
    fn buffer_age(&self) -> bool {
        self.presenter.capabilities().mode == PresentMode::BufferAge
    }

    /// Configures the presenter, the group targets and the kept target for a target of `size`.
    fn resize(&mut self, size: Size<i32, Device>) {
        let _ = self.presenter.configure(
            [size.width.max(1) as u32, size.height.max(1) as u32],
            self.target.is_some_and(|target| target.opaque),
        );
        self.groups.resize(size);
        let kept = kept_size(self.presenter.as_ref(), size);
        if self.kept.resize(&self.gpu, kept) {
            // The new target receives the old picture at the start of the next frame.
            self.kept_binding = bind_kept(&self.gpu, &self.pipelines.borrow(), &self.kept);
        }
    }
}

/// Returns the size of the kept target for a target of `size`.
// In buffer-age mode frames are composed into the presented image, so the kept target is unused.
fn kept_size(presenter: &dyn WgpuPresenter, size: Size<i32, Device>) -> Size<i32, Device> {
    if presenter.capabilities().mode == PresentMode::BufferAge {
        Size::new(1, 1)
    } else {
        size
    }
}

/// Returns the bind group the present copy reads the kept target through.
fn bind_kept(gpu: &Gpu, pipelines: &Pipelines, kept: &SceneTexture) -> wgpu::BindGroup {
    gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("iceblink.bind.kept"),
        layout: &pipelines.layouts().loaded,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(kept.view()),
        }],
    })
}

/// Returns the linear sampler that textured, filtered and external draws read through.
fn filtering_sampler(gpu: &Gpu) -> wgpu::Sampler {
    gpu.device().create_sampler(&wgpu::SamplerDescriptor {
        label: Some("iceblink.sampler.filtering"),
        // A filter's surround is cleared transparent inside its target, so clamping is enough.
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    })
}
