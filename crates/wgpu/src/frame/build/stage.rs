//! The blocks a plan stages for its draws to bind, and the frame values they are built from.

use iceblink::ShaderClock;
use iceblink::geom::{Device, Point, Size};

use crate::bind::globals::Globals;
use crate::effect::EffectKey;
use crate::frame::build::PlanBuilder;
use crate::frame::target::TargetRef;
use crate::pipeline::blur::BlurParams;
use crate::pipeline::composite::CompositeParams;
use crate::pipeline::effect_filter::EffectFilterParams;
use crate::pipeline::external::ExternalParams;
use crate::pipeline::marks::MarkDraw;
use crate::pipeline::vector::VectorInstance;
use crate::target::scale::TargetScale;

impl PlanBuilder<'_> {
    pub(crate) fn stage_blur(&mut self, params: &BlurParams) -> u32 {
        self.blocks.stage(params)
    }

    pub(crate) fn stage_effect_filter(&mut self, params: &EffectFilterParams) -> u32 {
        self.blocks.stage(params)
    }

    pub(crate) fn stage_composite(&mut self, params: &CompositeParams) -> u32 {
        self.blocks.stage(params)
    }

    pub(crate) fn stage_external(&mut self, params: &ExternalParams) -> u32 {
        self.blocks.stage(params)
    }

    /// Stages the mark draw block of the item at chunk `position`, and returns its offset.
    ///
    /// The block is built from `draw` the first time a position is staged, and later calls in the
    /// frame return the same offset.
    pub(crate) fn stage_mark_draw(
        &mut self,
        position: usize,
        draw: impl FnOnce() -> MarkDraw,
    ) -> u32 {
        if let Some(&offset) = self.mark_blocks.get(&position) {
            return offset;
        }
        let offset = self.mark_draws.stage(&draw());
        self.mark_blocks.insert(position, offset);
        offset
    }

    /// Stages vector composite `instances`, and returns the first and how many there are.
    pub(crate) fn stage_vector(
        &mut self,
        instances: impl IntoIterator<Item = VectorInstance>,
    ) -> (u32, u32) {
        self.vectors.stage(instances)
    }

    pub(crate) fn set_mark_extent(&mut self, extent: Size<i32, Device>) {
        self.mark_extent = extent;
    }

    pub(crate) fn shader_clock(&self) -> ShaderClock {
        self.shader_clock
    }

    pub(crate) fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    /// Returns the offset of a parameter slot's block, staged for this frame.
    pub(crate) fn effect_offset(&self, params: u32) -> Option<u32> {
        self.effect_offsets.get(params as usize).copied()
    }

    /// Returns the compiled shader in a frame shader slot.
    pub(crate) fn effect_key(&self, shader: u32) -> Option<EffectKey> {
        self.effects.get(shader)
    }

    /// Returns the offset of the globals block of `target`, staged once per frame for each kind,
    /// origin and extent.
    pub(super) fn globals_for(&mut self, target: TargetRef) -> u32 {
        let (kind, region) = match target {
            TargetRef::MarksPage(_) => (2, self.mark_extent),
            TargetRef::Composed => (0, self.region),
            TargetRef::Pool(slot) => (
                usize::from(slot.scale() == TargetScale::Half),
                slot.covered(),
            ),
        };
        let origin = target.origin();
        if origin == Point::ORIGIN
            && let Some((held, offset)) = self.staged_globals[kind]
            && held == region
        {
            return offset;
        }
        let key = (kind, origin, region);
        if let Some(&(_, offset)) = self.staged_other.iter().find(|(held, _)| *held == key) {
            return offset;
        }

        let block = Globals::for_target(region, target.scale(), self.subpixel_order)
            .with_frame(self.shader_clock, self.scale_factor)
            .with_origin(origin);
        let offset = self.globals.stage(&block);
        if origin == Point::ORIGIN && self.staged_globals[kind].is_none() {
            self.staged_globals[kind] = Some((region, offset));
        } else {
            self.staged_other.push((key, offset));
        }
        offset
    }
}
