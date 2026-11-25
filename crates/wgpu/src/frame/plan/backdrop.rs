//! Planning backdrops: the capture of what is beneath, its filters, and the composite back.

use iceblink::FrameBackdrop;
use iceblink::geom::{Device, Rect};

use crate::filter::chain::Chain;
use crate::frame::build::PlanBuilder;
use crate::frame::plan::bounds::{clamped, rounded_out};
use crate::frame::plan::composite::{composite, release_filtered};
use crate::frame::target::TargetRef;

/// Reports whether every filter of `backdrop` reads only the pixel it writes.
pub(super) fn reads_only_what_it_writes(backdrop: &FrameBackdrop) -> bool {
    backdrop
        .filters
        .iter()
        .all(|filter| filter.support() == [0.0; 4])
}

/// Plans `backdrop` over `beneath`, the target it reads and composites into.
///
/// Nothing is planned when the backdrop writes nothing within `scissor` and `used`, or when
/// `beneath` is a lease nothing was drawn into. A capture the pool has no target for counts as
/// unisolated.
pub(super) fn plan_backdrop(
    builder: &mut PlanBuilder<'_>,
    backdrop: &FrameBackdrop,
    beneath: TargetRef,
    scissor: Rect<i32, Device>,
    used: Rect<i32, Device>,
) {
    let written = rounded_out(backdrop.bounds);
    // The stream is replayed per rectangle, and a rectangle away from the backdrop writes none
    // of it and may not read what it would read.
    let bounds = clamped(written, scissor, used);
    if bounds.is_empty() {
        return;
    }
    let Some(source) = rounded_out(backdrop.source).intersection(used) else {
        return;
    };
    debug_assert!(
        beneath != TargetRef::Composed
            || reads_only_what_it_writes(backdrop)
            || scissor.contains_rect(source),
        "a backdrop reads {source:?}, which the region {scissor:?} being written does not contain"
    );

    // A group nothing was drawn into holds its last lease's texels, so what is beneath is
    // transparent.
    if let Some(slot) = beneath.slot() {
        builder.end_pass();
        if builder.is_unwritten(slot) {
            builder.begin_pass(beneath, scissor);
            return;
        }
    }

    let chain = Chain::of(&backdrop.filters);
    let Some((filtered, captured)) =
        crate::filter::backdrop::plan(builder, beneath, &chain, source)
    else {
        builder.note_unisolated();
        return;
    };
    composite(
        builder,
        &filtered,
        beneath,
        scissor,
        bounds,
        backdrop.backdrop.clip.0,
        1.0,
    );

    release_filtered(builder, &filtered);
    if let Some(slot) = captured.slot()
        && captured != filtered.target
        && !filtered
            .shadows
            .iter()
            .any(|shadow| shadow.source == captured)
    {
        builder.release(slot);
    }
}
