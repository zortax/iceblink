//! A filter shader: a pass that reads the group target and writes its replacement.

use iceblink::geom::{Device, Rect};

use crate::effect::EffectKey;
use crate::frame::build::PlanBuilder;
use crate::frame::segment::PlannedDraw;
use crate::frame::target::TargetRef;
use crate::pipeline::effect_filter::EffectFilterParams;
use crate::target::scale::TargetScale;

/// Plans a pass of `shader` with the parameter block at `block`, which reads `source` and writes
/// `region` into a new target.
///
/// Returns that target, or `None` when no target can be acquired.
pub(crate) fn plan(
    builder: &mut PlanBuilder<'_>,
    source: TargetRef,
    region: Rect<i32, Device>,
    shader: EffectKey,
    block: u32,
) -> Option<TargetRef> {
    let slot = builder.acquire(TargetScale::Full, region)?;
    let destination = TargetRef::Pool(slot);
    let params = builder.stage_effect_filter(&EffectFilterParams::new(
        region,
        (builder.extent_of(source), source.scale(), source.origin()),
        (
            builder.extent_of(destination),
            destination.scale(),
            destination.origin(),
        ),
        builder.shader_clock(),
        builder.scale_factor(),
    ));
    builder.begin_pass(destination, region);
    builder.draw(PlannedDraw::Effect {
        source,
        shader,
        params,
        block,
    });
    Some(destination)
}

/// Returns the offset of the parameter block in slot `params`, or `None` for a slot past the
/// frame's blocks.
pub(crate) fn block_of(builder: &PlanBuilder<'_>, params: u32) -> Option<u32> {
    builder.effect_offset(params)
}
