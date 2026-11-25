//! The counters of a built frame.

use iceblink::damage::DamageSet;
use iceblink::{DeviceRect, Frame, FrameCounters};

use crate::plan::PlanStats;

/// Returns the counters of `frame`, from what its walk and its plan counted and the shifts it
/// refused.
pub(crate) fn of(
    frame: &Frame,
    walked: FrameCounters,
    planned: &PlanStats,
    refusals: u32,
    output: DeviceRect,
) -> FrameCounters {
    FrameCounters {
        items_drawn: items_drawn(frame),
        items_culled: walked.items_culled + planned.culled,
        order_inserts: planned.order_inserts,
        vector_passes: frame.vector_plan.passes.len() as u32,
        mark_bins: (frame.mark_plan.bins.len() + frame.mark_plan.overflow.len()) as u32,
        damage_px: pixels(&frame.damage.draw, output),
        shifted_px: frame
            .shifts
            .iter()
            .filter_map(|shift| shift.destination())
            .map(|rect| i64::from(rect.size.width) * i64::from(rect.size.height))
            .sum::<i64>() as u64,
        shift_refusals: refusals,
        ..walked
    }
}

/// Counts the instances in a frame's lanes and arrays.
fn items_drawn(frame: &Frame) -> u32 {
    let lanes = &frame.lanes;
    (lanes.quads.drawn()
        + lanes.shader_quads.drawn()
        + lanes.shadows.drawn()
        + lanes.lines.drawn()
        + lanes.mono.drawn()
        + lanes.subpixel.drawn()
        + lanes.color.drawn()
        + lanes.marks.drawn()
        + frame.paths.len()
        + frame.externals.len()
        + frame.backdrops.len()) as u32
}

/// Counts the pixels of a damage set, the whole output when it is full.
fn pixels(damage: &DamageSet, output: DeviceRect) -> u64 {
    match damage.area() {
        Some(area) => area.max(0) as u64,
        None => (i64::from(output.size.width) * i64::from(output.size.height)).max(0) as u64,
    }
}
