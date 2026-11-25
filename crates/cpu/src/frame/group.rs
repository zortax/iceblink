//! Opening a group's canvas, and compositing it into the target beneath when the group ends.

use iceblink::geom::{Device, Edges, Rect};
use iceblink::{BlendMode, FrameFilter};

use crate::frame::filter::{self, Chain, Composite, EdgeMode};
use crate::frame::pool::Pool;
use crate::frame::rects::rounded_out;
use crate::frame::{Context, Worker};
use crate::target::{Canvas, Precision, Surface};

/// The slack for each filter step beyond the kernel's own support.
pub(crate) const STEP_SLACK: f32 = 4.0;

/// The slack for each resampling step beyond the kernel's own support.
pub(crate) const RESAMPLE_SLACK: f32 = 12.0;

/// The farthest a region is grown for the filters and backdrops that read it.
pub(crate) const MAX_REACH: i32 = 1 << 16;

/// A group being drawn.
#[derive(Debug)]
pub(crate) struct Open {
    /// The canvas the group's content is drawn into, or `None` for a group outside the scissor.
    pub(crate) canvas: Option<Canvas>,
    pub(crate) group: usize,
    /// The device pixels the canvas covers.
    pub(crate) region: Rect<i32, Device>,
}

/// Opens group `group` for a replay cut to `scissor`, `depth` groups deep.
///
/// The canvas covers the group's source, cut to the scissor grown by how far the group's filters
/// and the backdrops inside it read. A group outside the scissor opens no canvas and an empty
/// region, so its content draws nothing.
pub(crate) fn begin(
    context: &Context<'_>,
    pool: &mut Pool,
    group: usize,
    scissor: Rect<i32, Device>,
    depth: usize,
) -> Open {
    let nothing = Open {
        canvas: None,
        group,
        region: Rect::ZERO,
    };
    let Some(boundary) = context.frame.groups.get(group) else {
        return nothing;
    };
    let whole = rounded_out(boundary.source);
    if !whole.intersects(scissor) {
        return nothing;
    }
    let nested = context.nested.get(group).copied().unwrap_or(0);
    let reach = chain_reach(&boundary.filters).saturating_add(nested);
    let region = whole
        .intersection(scissor.outset(Edges::uniform(reach.min(MAX_REACH))))
        .unwrap_or(whole);
    let plain = depth == 0 && boundary.filters.is_empty() && boundary.blend == BlendMode::NORMAL;
    let precision = if plain {
        Precision::Byte
    } else {
        Precision::Float
    };
    Open {
        canvas: Some(pool.acquire(region, precision, false)),
        group,
        region,
    }
}

/// Ends `open`: runs its filter chain and composites the result into `beneath`, cut to
/// `scissor`, with the group's clip, opacity and blend mode.
pub(crate) fn end(
    context: &Context<'_>,
    worker: &mut Worker,
    beneath: &mut Surface<'_>,
    open: Open,
    scissor: Rect<i32, Device>,
) {
    let Some(canvas) = open.canvas else {
        return;
    };
    let group = &context.frame.groups[open.group];
    let chain = Chain::of(&group.filters);
    let filtered = filter::run(
        &mut worker.pool,
        &chain,
        canvas,
        open.region,
        open.region,
        EdgeMode::Transparent,
    );
    let how = Composite {
        bounds: rounded_out(group.bounds),
        clip: group.clip,
        opacity: group.opacity,
        blend: group.blend,
    };
    filter::composite(
        &context.scene,
        beneath,
        scissor,
        &filtered,
        how,
        &mut worker.row,
    );
    filtered.release(&mut worker.pool);
    worker.counts.draws += 1;
}

/// Returns how far a group's filters read past the region they write, with slack for each step.
pub(crate) fn chain_reach(filters: &[FrameFilter]) -> i32 {
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
