//! Planning groups: the region each one draws into, and the composite at its end.

use iceblink::geom::{Device, Edges, Rect};
use iceblink::{BatchKind, BlendMode, Frame, FrameFilter, FrameGroup};

use crate::filter::chain::Chain;
use crate::filter::{self, EdgeMode};
use crate::frame::build::PlanBuilder;
use crate::frame::plan::PlanInputs;
use crate::frame::plan::backdrop::reads_only_what_it_writes;
use crate::frame::plan::bounds::rounded_out;
use crate::frame::plan::composite::{composite, release_filtered};
use crate::frame::target::TargetRef;
use crate::target::scale::TargetScale;

// Slack for each filter step, and for each resampling step, beyond the kernel's own support.
const STEP_SLACK: f32 = 4.0;
const RESAMPLE_SLACK: f32 = 12.0;

/// The farthest a group's region reaches past the rectangle, in device pixels.
const MAX_REACH: i32 = 1 << 16;

/// A group whose content is being planned.
pub(super) struct OpenGroup {
    /// The target the content draws into, or `None` for a group the rectangle misses.
    pub(super) target: Option<TargetRef>,
    /// The target the group composites into.
    pub(super) parent: TargetRef,
    /// The group's index in the frame.
    pub(super) group: usize,
    /// The device region the content is drawn over.
    pub(super) region: Rect<i32, Device>,
}

/// Opens `group` over the part of its source that `scissor` can reach through its filters, grown
/// by `nested` for the backdrops inside it.
///
/// A group the scissor misses opens with no target and an empty region. Returns `None` when the
/// pool has no target to lend.
pub(super) fn begin_group(
    builder: &mut PlanBuilder<'_>,
    inputs: &PlanInputs<'_>,
    group: usize,
    parent: TargetRef,
    scissor: Rect<i32, Device>,
    nested: i32,
    plain: bool,
) -> Option<OpenGroup> {
    let boundary = &inputs.frame.groups[group];
    let whole = rounded_out(boundary.source);
    if !whole.intersects(scissor) {
        return Some(OpenGroup {
            target: None,
            parent,
            group,
            region: Rect::ZERO,
        });
    }

    let reach = chain_reach(&boundary.filters).saturating_add(nested);
    let region = whole
        .intersection(scissor.outset(Edges::uniform(reach.min(MAX_REACH))))
        .unwrap_or(whole);
    let target = TargetRef::Pool(builder.acquire_group(TargetScale::Full, region, plain)?);
    Some(OpenGroup {
        target: Some(target),
        parent,
        group,
        region,
    })
}

/// Closes `open`, whose content was drawn into `target`, by filtering it and compositing the
/// result into its parent.
///
/// A target nothing was drawn into is released without a composite.
pub(super) fn end_group(
    builder: &mut PlanBuilder<'_>,
    frame: &Frame,
    open: &OpenGroup,
    target: TargetRef,
    scissor: Rect<i32, Device>,
) {
    // Closed here, so an unwritten lease is known before its composite is planned.
    builder.end_pass();
    if let Some(slot) = target.slot()
        && builder.is_unwritten(slot)
    {
        // The lease still holds what its last lease left there.
        builder.release(slot);
        return;
    }

    let group = &frame.groups[open.group];
    let chain = Chain::of(&group.filters);
    let filtered = filter::plan(
        builder,
        &chain,
        target,
        open.region,
        open.region,
        EdgeMode::Transparent,
    );
    if group.blend != BlendMode::NORMAL {
        builder.note_unsupported_blend();
    }
    composite(
        builder,
        &filtered,
        open.parent,
        scissor,
        rounded_out(group.bounds),
        group.clip,
        group.opacity,
    );

    release_filtered(builder, &filtered);
    if let Some(slot) = target.slot() {
        builder.release(slot);
    }
}

/// Reports whether a group draws its content unchanged: no filters and normal blending.
pub(super) fn is_plain(group: &FrameGroup) -> bool {
    group.filters.is_empty() && group.blend == BlendMode::NORMAL
}

/// Returns how far, in device pixels, the output of `filters` reads its input.
fn chain_reach(filters: &[FrameFilter]) -> i32 {
    let reach: f32 = filters
        .iter()
        .map(|filter| {
            let [left, top, right, bottom] = filter.support();
            let support = left.max(top).max(right).max(bottom);
            if support > 0.0 {
                STEP_SLACK + support + RESAMPLE_SLACK
            } else {
                STEP_SLACK
            }
        })
        .sum();
    (reach + STEP_SLACK).ceil().min(MAX_REACH as f32) as i32
}

/// Returns, for each group of `frame`, how far its region must reach so that every backdrop inside
/// it reads written texels.
pub(super) fn backdrop_reach(frame: &Frame) -> Vec<i32> {
    let mut reach = vec![0; frame.groups.len()];
    let mut open: Vec<usize> = Vec::new();
    for batch in &frame.batches {
        match batch.kind {
            BatchKind::GroupBegin(group) => open.push(group as usize),
            BatchKind::GroupEnd(_) => {
                open.pop();
            }
            BatchKind::Backdrop(index) => {
                let Some(backdrop) = frame.backdrops.get(index as usize) else {
                    continue;
                };
                if open.is_empty() || reads_only_what_it_writes(backdrop) {
                    continue;
                }
                let [wx, wy, ww, wh] = backdrop.bounds;
                let [rx, ry, rw, rh] = backdrop.source;
                let outset = (wx - rx)
                    .max(wy - ry)
                    .max((rx + rw) - (wx + ww))
                    .max((ry + rh) - (wy + wh))
                    .max(0.0);
                let needed = (outset + STEP_SLACK + RESAMPLE_SLACK)
                    .ceil()
                    .min(MAX_REACH as f32) as i32;
                for &group in &open {
                    if let Some(held) = reach.get_mut(group) {
                        *held = (*held).max(needed);
                    }
                }
            }
            _ => {}
        }
    }
    reach
}
