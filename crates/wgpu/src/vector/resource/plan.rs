//! A frame's vector plan with a rasteriser's resources attached.

use core::ops::Range;

use iceblink::{DeviceRect, PlannedPath, VectorPlan};

use crate::vector::resource::{Layering, VectorTarget};

/// A frame's vector plan with a rasteriser's resources attached.
///
/// The plan holds one pass per planned pass, in the same order, or none when the frame has no
/// passes. The items are the vector plan's items, unchanged.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResourcedPlan {
    /// The passes, in plan order.
    pub passes: Vec<ResourcedPass>,
    /// Every pass's paths, grouped by pass.
    pub items: Vec<PlannedPath>,
}

impl ResourcedPlan {
    /// Starts the resourced plan of `plan`, with its items and no passes yet.
    pub fn resourcing(plan: &VectorPlan) -> Self {
        Self {
            passes: Vec::with_capacity(plan.passes.len()),
            items: plan.items.clone(),
        }
    }

    /// Resources `plan` with the placements of `layering`, which was packed from the regions of
    /// `plan`'s passes in order.
    ///
    /// The result holds one pass per planned pass. A pass that `layering` has no placement for
    /// gets [`VectorTarget::NONE`]. A pass is `clamped` when the planned pass is clamped or
    /// overhangs.
    pub fn placed(plan: &VectorPlan, layering: &Layering) -> Self {
        let mut resourced = Self::resourcing(plan);
        resourced
            .passes
            .extend(plan.passes.iter().enumerate().map(|(index, planned)| {
                let placement = layering.placement(index);
                ResourcedPass {
                    region: planned.region,
                    raster_region: placement.map_or(planned.region, |placement| placement.bin),
                    target: placement.map_or(VectorTarget::NONE, |placement| placement.target),
                    round: placement.map_or(layering.rounds(), |placement| placement.round),
                    items: planned.items.clone(),
                    clip: planned.clip,
                    instanced: planned.instanced,
                    clamped: planned.clamped || planned.overhangs,
                }
            }));
        resourced
    }

    /// Reports whether the plan has no passes.
    pub fn is_empty(&self) -> bool {
        self.passes.is_empty()
    }

    /// Returns how many rounds the passes are rasterised in.
    pub fn rounds(&self) -> usize {
        self.passes.last().map_or(0, |pass| pass.round + 1)
    }

    /// Returns the indices of the passes of `round`.
    pub fn round(&self, round: usize) -> Range<usize> {
        let start = self.passes.partition_point(|pass| pass.round < round);
        let end = self.passes.partition_point(|pass| pass.round <= round);
        start..end.max(start)
    }

    /// Replaces `into` with the targets the passes of `round` write.
    ///
    /// The targets come out sorted, each once, and without [`VectorTarget::NONE`].
    pub fn targets_of(&self, round: usize, into: &mut Vec<VectorTarget>) {
        into.clear();
        into.extend(
            self.passes[self.round(round)]
                .iter()
                .map(|pass| pass.target)
                .filter(|target| *target != VectorTarget::NONE),
        );
        into.sort_unstable();
        into.dedup();
    }

    /// Returns the paths of `pass`.
    pub fn items_of(&self, pass: &ResourcedPass) -> &[PlannedPath] {
        &self.items[pass.items.start as usize..pass.items.end as usize]
    }
}

/// One planned pass with a rasteriser's resources attached.
///
/// Every field except `raster_region`, `target` and `round` is copied from the planned pass.
#[derive(Clone, Debug, PartialEq)]
pub struct ResourcedPass {
    /// The device region the pass covers.
    pub region: DeviceRect,
    /// Where in `target` the pass's pixels are.
    pub raster_region: DeviceRect,
    /// The scratch target the pass is rasterised into.
    pub target: VectorTarget,
    /// The round that rasterises the pass.
    ///
    /// A pass is never in an earlier round than the pass before it.
    pub round: usize,
    /// The pass's paths, as a range of the plan's items.
    pub items: Range<u32>,
    /// The frame clip slot a whole-pass composite binds.
    pub clip: u32,
    /// Whether the pass may be composited one path at a time.
    pub instanced: bool,
    /// Whether the paths reach past `region`, so the rasteriser keeps them inside
    /// `raster_region`.
    pub clamped: bool,
}
