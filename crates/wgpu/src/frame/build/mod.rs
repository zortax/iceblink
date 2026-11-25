//! Building a frame's plan: passes, draws, staged blocks and lent targets.

mod lend;
mod passes;
mod stage;

use iceblink::geom::{Device, Point, Rect, Size};
use iceblink::{ShaderClock, SubpixelOrder};
use rustc_hash::FxHashMap;

use crate::buffer::slots::SlotBuffer;
use crate::buffer::vectors::VectorInstances;
use crate::effect::EffectSlots;
use crate::frame::segment::{PlannedDraw, PlannedPass, Segment};
use crate::gpu::device::Gpu;
use crate::target::group_pool::{GroupPool, GroupSlot};

/// A frame's plan, recorded once per run of segments between vector rounds.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct FramePlan {
    pub(crate) segments: Vec<Segment>,
    /// The draws of every pass, as the passes index them.
    pub(crate) draws: Vec<PlannedDraw>,
    /// Draws left out because what they draw is missing or cannot be drawn into their target.
    pub(crate) deferred: u32,
    /// Groups composited with normal blending because their blend mode is not supported.
    pub(crate) unsupported_blends: u32,
    /// Groups and backdrops drawn without a target of their own, because the pool had none to
    /// lend.
    pub(crate) unisolated: u32,
    /// Batches left out because their ink misses the rectangle being drawn.
    pub(crate) culled: u32,
    /// The draw rectangles the plan covers.
    pub(crate) rects: Vec<Rect<i32, Device>>,
}

impl FramePlan {
    pub(crate) fn draws_of(&self, pass: &PlannedPass) -> &[PlannedDraw] {
        &self.draws[pass.draws.clone()]
    }

    /// Returns the runs of segments between round markers, each with the vector round prepared
    /// before it, if there is one.
    pub(crate) fn runs(&self) -> impl Iterator<Item = (Option<usize>, &[Segment])> {
        let rounds =
            core::iter::once(None).chain(self.segments.iter().filter_map(Segment::round).map(Some));
        rounds.zip(self.segments.split(|segment| segment.round().is_some()))
    }
}

/// The target kind, origin and extent a globals block describes.
type GlobalsKey = (usize, Point<i32, Device>, Size<i32, Device>);

/// Builds a [`FramePlan`], staging the blocks its draws bind and lending the targets its groups
/// draw into.
pub(crate) struct PlanBuilder<'gpu> {
    gpu: &'gpu Gpu,
    pool: &'gpu mut GroupPool,
    globals: &'gpu mut SlotBuffer,
    blocks: &'gpu mut SlotBuffer,
    vectors: &'gpu mut VectorInstances,
    mark_draws: &'gpu mut SlotBuffer,
    /// The staged mark draw block of each drawn chunk position.
    mark_blocks: FxHashMap<usize, u32>,
    mark_extent: Size<i32, Device>,
    subpixel_order: SubpixelOrder,
    shader_clock: ShaderClock,
    scale_factor: f32,
    effect_offsets: &'gpu [u32],
    effects: &'gpu EffectSlots,
    /// The extent of the composed target the frame uses.
    region: Size<i32, Device>,
    composed_format: wgpu::TextureFormat,
    composed_extent: Size<i32, Device>,
    /// The globals blocks staged at the origin, one per kind: full scale, half scale and mark
    /// pages.
    staged_globals: [Option<(Size<i32, Device>, u32)>; 3],
    /// Every other staged globals block.
    staged_other: Vec<(GlobalsKey, u32)>,
    plan: FramePlan,
    /// The pass draws are planned into.
    open: Option<PlannedPass>,
    /// The slots lent and not yet released, which the finished plan releases.
    lent: Vec<GroupSlot>,
    /// The lent slots no pass has written yet.
    unwritten: Vec<GroupSlot>,
    /// The vector round the open segments follow.
    round: usize,
}

/// The buffers, slots and targets a plan stages into and draws from.
pub(crate) struct PlanResources<'gpu> {
    pub(crate) pool: &'gpu mut GroupPool,
    pub(crate) globals: &'gpu mut SlotBuffer,
    pub(crate) blocks: &'gpu mut SlotBuffer,
    pub(crate) vectors: &'gpu mut VectorInstances,
    pub(crate) mark_draws: &'gpu mut SlotBuffer,
    pub(crate) effect_offsets: &'gpu [u32],
    pub(crate) effects: &'gpu EffectSlots,
}

/// The target a frame composes into.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ComposedTarget {
    /// The extent the frame uses.
    pub(crate) region: Size<i32, Device>,
    pub(crate) format: wgpu::TextureFormat,
    /// The extent the target is allocated at.
    pub(crate) extent: Size<i32, Device>,
}

impl<'gpu> PlanBuilder<'gpu> {
    pub(crate) fn new(
        gpu: &'gpu Gpu,
        resources: PlanResources<'gpu>,
        subpixel_order: SubpixelOrder,
        (shader_clock, scale_factor): (ShaderClock, f32),
        composed: ComposedTarget,
    ) -> Self {
        let PlanResources {
            pool,
            globals,
            blocks,
            vectors,
            mark_draws,
            effect_offsets,
            effects,
        } = resources;
        Self {
            gpu,
            pool,
            globals,
            blocks,
            vectors,
            mark_draws,
            mark_blocks: FxHashMap::default(),
            mark_extent: Size::new(0, 0),
            subpixel_order,
            shader_clock,
            scale_factor,
            effect_offsets,
            effects,
            region: composed.region,
            composed_format: composed.format,
            composed_extent: composed.extent,
            staged_globals: [None; 3],
            staged_other: Vec::new(),
            plan: FramePlan::default(),
            open: None,
            lent: Vec::new(),
            unwritten: Vec::new(),
            round: 0,
        }
    }

    /// Counts a draw left out of the plan.
    pub(crate) fn defer(&mut self) {
        self.plan.deferred += 1;
    }

    pub(crate) fn note_unsupported_blend(&mut self) {
        self.plan.unsupported_blends += 1;
    }

    pub(crate) fn note_unisolated(&mut self) {
        self.plan.unisolated += 1;
    }

    pub(crate) fn note_culled(&mut self) {
        self.plan.culled += 1;
    }

    /// Closes the open pass, releases every slot still lent, and returns the plan.
    pub(crate) fn finish(mut self) -> FramePlan {
        self.end_pass();
        for slot in core::mem::take(&mut self.lent) {
            self.pool.release(slot);
        }
        self.plan
    }
}
