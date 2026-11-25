//! Filter chains planned over group targets.
//!
//! Each step writes a region grown by the reach of the steps after it, so the last step's output
//! is whole. The composite applies a trailing colour matrix.

pub(crate) mod backdrop;
pub(crate) mod blur;
pub(crate) mod chain;
pub(crate) mod drop_shadow;
pub(crate) mod effect;
pub(crate) mod matrix;

use iceblink::geom::{Device, Edges, Rect};

use crate::filter::chain::{Chain, Step};
use crate::filter::matrix::ColorMatrix;
use crate::frame::build::PlanBuilder;
use crate::frame::segment::PlannedDraw;
use crate::frame::target::TargetRef;
use crate::pipeline::composite::CompositeParams;
use crate::target::scale::TargetScale;

/// The pixels a blur reads outside the pixels its source holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EdgeMode {
    /// Transparent black.
    Transparent,
    /// The nearest pixel the source holds.
    Duplicate,
}

/// A drop shadow drawn behind the filtered content: the target it is read from and its composite
/// block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShadowLayer {
    pub(crate) source: TargetRef,
    pub(crate) params: CompositeParams,
}

/// A planned filter chain.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Filtered {
    /// The target that holds the filtered content.
    pub(crate) target: TargetRef,
    /// The device pixels `target` holds.
    pub(crate) holds: Rect<i32, Device>,
    /// The colour matrix the composite applies.
    pub(crate) matrix: ColorMatrix,
    /// The drop shadows drawn behind the content, in chain order.
    pub(crate) shadows: Vec<ShadowLayer>,
}

/// Plans `chain` over `source`, which holds `valid`, so that the result is whole over `output`.
///
/// Blurs and drop shadows read outside `valid` as `edge` says.
pub(crate) fn plan(
    builder: &mut PlanBuilder<'_>,
    chain: &Chain,
    source: TargetRef,
    output: Rect<i32, Device>,
    valid: Rect<i32, Device>,
    edge: EdgeMode,
) -> Filtered {
    let (steps, folded) = chain.split();
    let regions = staging(steps, output, valid);
    let mut filtered = Filtered {
        target: source,
        holds: valid,
        matrix: folded,
        shadows: Vec::new(),
    };
    // The device pixels the next step's source holds. A skipped step leaves them unchanged.
    let mut holds = valid;
    for (step, writes) in steps.iter().zip(regions) {
        let next = match *step {
            Step::Matrix(matrix) => materialise(builder, filtered.target, writes, matrix),
            Step::Blur(deviation) => {
                blur::plan(builder, filtered.target, writes, holds, deviation, edge)
                    .map(|blurred| blurred.target)
            }
            Step::Custom { shader, params, .. } => {
                custom(builder, filtered.target, writes, shader, params)
            }
            Step::DropShadow {
                offset_x,
                offset_y,
                blur,
                color,
            } => {
                // A shadow is a layer drawn behind the content, so it neither advances the chain's
                // target nor what that target holds.
                let shadow = drop_shadow::Shadow {
                    offset: (offset_x, offset_y),
                    deviation: blur,
                    color,
                };
                filtered.shadows.extend(drop_shadow::plan(
                    builder,
                    filtered.target,
                    writes,
                    holds,
                    shadow,
                    edge,
                ));
                None
            }
        };
        if let Some(next) = next {
            replace(builder, &mut filtered, source, next);
            holds = writes;
        }
    }
    filtered.holds = holds;
    filtered
}

/// Returns the region each of `steps` writes: `output` grown by the reach of the steps after it,
/// cut to `valid`.
fn staging(
    steps: &[Step],
    output: Rect<i32, Device>,
    valid: Rect<i32, Device>,
) -> Vec<Rect<i32, Device>> {
    let mut regions = Vec::with_capacity(steps.len());
    let mut beyond = 0;
    for step in steps.iter().rev() {
        regions.push(
            output
                .outset(Edges::uniform(beyond))
                .intersection(valid)
                .unwrap_or(output),
        );
        beyond += reach_of(step);
    }
    regions.reverse();
    regions
}

/// Returns how far past the pixel it writes a step reads.
fn reach_of(step: &Step) -> i32 {
    match step {
        Step::Matrix(_) => 0,
        Step::Custom { reach, .. } => reach.ceil() as i32,
        Step::Blur(deviation) => blur::reach(*deviation),
        Step::DropShadow {
            offset_x,
            offset_y,
            blur,
            ..
        } => blur::reach(*blur) + offset_x.abs().max(offset_y.abs()).ceil() as i32,
    }
}

/// Makes `next` the chain's target, and releases the target it replaces unless that is `input`.
fn replace(
    builder: &mut PlanBuilder<'_>,
    filtered: &mut Filtered,
    input: TargetRef,
    next: TargetRef,
) {
    let previous = filtered.target;
    filtered.target = next;
    if previous != input
        && previous != next
        && let Some(slot) = previous.slot()
    {
        builder.release(slot);
    }
}

/// Plans a pass of the application shader in slot `shader` with the parameters at `params`, and
/// returns the target it writes.
///
/// Returns `None` when the parameters or the shader are missing, or no target can be acquired.
fn custom(
    builder: &mut PlanBuilder<'_>,
    source: TargetRef,
    region: Rect<i32, Device>,
    shader: u32,
    params: u32,
) -> Option<TargetRef> {
    let block = effect::block_of(builder, params)?;
    let shader = builder.effect_key(shader)?;
    effect::plan(builder, source, region, shader, block)
}

/// Plans a pass that applies `matrix` to `source` over `region`, and returns the target it writes,
/// or `None` when no target can be acquired.
fn materialise(
    builder: &mut PlanBuilder<'_>,
    source: TargetRef,
    region: Rect<i32, Device>,
    matrix: ColorMatrix,
) -> Option<TargetRef> {
    let slot = builder.acquire(TargetScale::Full, region)?;
    let destination = TargetRef::Pool(slot);
    let params = CompositeParams::new(
        region.to_unit(),
        builder.extent_of(source),
        source.scale(),
        source.origin(),
        region,
        iceblink::FRAME_NO_CLIP,
    )
    .with_matrix(matrix);
    let params = builder.stage_composite(&params);
    builder.begin_pass(destination, region);
    builder.draw(PlannedDraw::Composite { source, params });
    Some(destination)
}
