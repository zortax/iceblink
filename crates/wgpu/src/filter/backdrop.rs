//! A backdrop: the target beneath it, captured and filtered.

use iceblink::geom::{Device, Rect};

use crate::filter::chain::Chain;
use crate::filter::{self, EdgeMode, Filtered};
use crate::frame::build::PlanBuilder;
use crate::frame::segment::{EncoderOp, PlannedDraw};
use crate::frame::target::TargetRef;

/// Captures `region` of `beneath` into a new target and plans `chain` over the capture.
///
/// Returns the filtered chain and the capture, or `None` when no target can be acquired.
pub(crate) fn plan(
    builder: &mut PlanBuilder<'_>,
    beneath: TargetRef,
    chain: &Chain,
    region: Rect<i32, Device>,
) -> Option<(Filtered, TargetRef)> {
    let slot = builder.acquire_like(beneath, region)?;
    let captured = TargetRef::Pool(slot);
    if let Some(under) = beneath.slot()
        && !under.holds(region)
    {
        builder.begin_pass(captured, region);
        builder.draw(PlannedDraw::Clear);
        builder.end_pass();
    }
    builder.encoder(EncoderOp::Capture {
        source: beneath,
        destination: captured,
        region,
    });
    Some((
        filter::plan(
            builder,
            chain,
            captured,
            region,
            region,
            EdgeMode::Duplicate,
        ),
        captured,
    ))
}
