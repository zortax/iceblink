//! Planning a frame's batches into passes, once per draw rectangle.
//!
//! A group opens a target from the pool and draws its content into it, over its whole written
//! region cut to what the rectangle can reach through the group's filters. Its end runs the filter
//! chain and composites the result into the target beneath. A backdrop captures what is beneath
//! it, filters the copy and composites it back. When a frame has several rectangles, batches whose
//! ink misses the rectangle are culled. An opaque cover that holds the rectangle starts the replay
//! at its quad.

mod backdrop;
mod bounds;
mod composite;
mod group;
mod marks;

use iceblink::damage::DamageSet;
use iceblink::geom::{Device, Rect};
use iceblink::{Batch, BatchKind, Frame, FrameMarks, OpaqueCover};

use crate::frame::build::{FramePlan, PlanBuilder};
use crate::frame::damage;
use crate::frame::segment::PlannedDraw;
use crate::frame::target::TargetRef;
use crate::frame::vector;
use crate::pipeline::external::ExternalParams;
use crate::vector::ResourcedPlan;

use self::backdrop::{plan_backdrop, reads_only_what_it_writes};
use self::bounds::rounded_out;
use self::group::{OpenGroup, backdrop_reach, begin_group, end_group, is_plain};
use self::marks::{plan_mark_pages, plan_marks};

/// What planning reads beside the frame.
pub(crate) struct PlanInputs<'a> {
    pub(crate) frame: &'a Frame,
    /// The marks item at a drawn chunk position, if there is one.
    pub(crate) marks: &'a dyn Fn(usize) -> Option<FrameMarks>,
    /// The first shape cell of the marks payload at a drawn chunk position.
    pub(crate) cells: &'a dyn Fn(usize) -> u32,
    /// Whether the texture of an external quad is premultiplied, or `None` if it is not bound.
    pub(crate) externals: &'a dyn Fn(u32) -> Option<bool>,
    /// The vector rasteriser's plan, if the renderer has a rasteriser.
    pub(crate) vectors: Option<&'a ResourcedPlan>,
    /// The pixels of the composed target in use.
    pub(crate) used: Rect<i32, Device>,
}

/// Plans every draw rectangle of `rects` and returns the plan.
///
/// A `full` frame redraws everything, so no opaque cover shortens its replay.
pub(crate) fn plan_segments(
    mut builder: PlanBuilder<'_>,
    inputs: &PlanInputs<'_>,
    rects: Vec<Rect<i32, Device>>,
    full: bool,
) -> FramePlan {
    let frame = inputs.frame;
    plan_mark_pages(&mut builder, inputs, &frame.mark_plan);

    let nested = if frame.backdrops.is_empty() {
        Vec::new()
    } else {
        backdrop_reach(frame)
    };
    // A cover is its fill inset by a pixel, so none holds a frame that redraws everything.
    let covers: &[OpaqueCover] = if full { &[] } else { &frame.opaque_covers };
    let cull = rects.len() > 1;
    for &rect in &rects {
        plan_rect(&mut builder, inputs, &nested, covers, rect, cull);
    }

    let mut plan = builder.finish();
    plan.rects = rects;
    plan
}

/// Returns the draw rectangles of `draw` within `used`, grown to hold what every backdrop the
/// damage reaches reads.
pub(crate) fn draw_rects(
    frame: &Frame,
    draw: &DamageSet,
    used: Rect<i32, Device>,
) -> Vec<Rect<i32, Device>> {
    let backdrops: Vec<_> = frame
        .backdrops
        .iter()
        .filter(|backdrop| !reads_only_what_it_writes(backdrop))
        .map(|backdrop| rounded_out(backdrop.source))
        .collect();
    damage::rects_covering_backdrops(draw, used, &backdrops)
}

/// Returns the part of `batch` an opaque cover leaves to draw: nothing before the cover's batch,
/// and the cover's own batch from its quad.
fn behind(index: usize, batch: &Batch, cover: Option<&OpaqueCover>) -> Option<Batch> {
    let Some(cover) = cover else {
        return Some(batch.clone());
    };
    let first = cover.batch as usize;
    if index < first {
        return None;
    }
    if index == first {
        let start = cover.at.max(batch.range.start);
        return (start < batch.range.end).then(|| Batch {
            range: start..batch.range.end,
            ..batch.clone()
        });
    }
    Some(batch.clone())
}

/// Plans the replay of the frame's batches over `rect`.
///
/// `nested` holds the reach each group needs for the backdrops inside it.
fn plan_rect(
    builder: &mut PlanBuilder<'_>,
    inputs: &PlanInputs<'_>,
    nested: &[i32],
    covers: &[OpaqueCover],
    rect: Rect<i32, Device>,
    cull: bool,
) {
    let frame = inputs.frame;
    let mut stack: Vec<OpenGroup> = Vec::new();
    let mut current = TargetRef::Composed;
    let mut scissor = rect;
    let mut depth = 0usize;

    builder.begin_pass(current, scissor);
    // The latest cover that holds the rectangle hides the clear and everything before it.
    let hidden = covers
        .iter()
        .filter(|cover| cover.rect.contains_rect(rect))
        .max_by_key(|cover| (cover.batch, cover.at));
    if hidden.is_none() {
        builder.draw(PlannedDraw::Clear);
    }

    for (index, batch) in frame.batches.iter().enumerate() {
        let Some(batch) = behind(index, batch, hidden) else {
            continue;
        };
        match batch.kind {
            BatchKind::GroupBegin(group) => {
                let Some(boundary) = frame.groups.get(group as usize) else {
                    continue;
                };
                let reach = nested.get(group as usize).copied().unwrap_or(0);
                let plain = depth == 0 && is_plain(boundary);
                depth += 1;
                match begin_group(
                    builder,
                    inputs,
                    group as usize,
                    current,
                    scissor,
                    reach,
                    plain,
                ) {
                    Some(open) => {
                        scissor = open.region;
                        current = open.target.unwrap_or(current);
                        stack.push(open);
                        builder.begin_pass(current, scissor);
                    }
                    // The pool had nothing to lend, so the content is drawn into the target
                    // beneath.
                    None => builder.note_unisolated(),
                }
            }
            BatchKind::GroupEnd(_) => {
                depth = depth.saturating_sub(1);
                let Some(open) = stack.pop() else {
                    continue;
                };
                // A composite into an enclosing group writes that group's whole region, so the
                // enclosing filter reads the same texels in a partial redraw as in a whole one.
                scissor = stack.last().map_or(rect, |outer| outer.region);
                if let Some(target) = open.target {
                    end_group(builder, frame, &open, target, scissor);
                }
                current = open.parent;
                builder.begin_pass(current, scissor);
            }
            BatchKind::Backdrop(index) => {
                let Some(backdrop) = frame.backdrops.get(index as usize) else {
                    continue;
                };
                plan_backdrop(builder, backdrop, current, scissor, inputs.used);
                builder.begin_pass(current, scissor);
            }
            BatchKind::External(index) => plan_external(builder, inputs, index),
            BatchKind::Paths { pass } => plan_paths(builder, inputs, pass),
            BatchKind::Marks => {
                plan_marks(builder, inputs, batch.range.clone(), current, scissor);
            }
            _ if cull && !batch.ink.intersects(scissor) => builder.note_culled(),
            // Per-channel coverage blends correctly only against an opaque destination.
            BatchKind::Subpixel { .. } if current.slot().is_some() => builder.defer(),
            _ => builder.draw(PlannedDraw::Batch(batch)),
        }
    }

    // Markers pair up in a valid frame, so this closes nothing there.
    while let Some(open) = stack.pop() {
        let enclosing = stack.last().map_or(rect, |outer| outer.region);
        if let Some(target) = open.target {
            end_group(builder, frame, &open, target, enclosing);
        }
        builder.begin_pass(open.parent, enclosing);
    }
    builder.end_pass();
}

/// Plans the draw of external quad `index`.
///
/// A quad whose texture is not bound draws nothing and counts as deferred.
fn plan_external(builder: &mut PlanBuilder<'_>, inputs: &PlanInputs<'_>, index: u32) {
    let Some(external) = inputs.frame.externals.get(index as usize) else {
        return;
    };
    match (inputs.externals)(index) {
        Some(premultiplied) => {
            let params = builder.stage_external(&ExternalParams::of(&external.quad, premultiplied));
            builder.draw(PlannedDraw::External { index, params });
        }
        None => builder.defer(),
    }
}

/// Plans the composite of vector pass `pass`, in the round that rasterises it.
///
/// A pass with nothing to composite counts as deferred, as does every pass when the renderer has
/// no rasteriser or the rasteriser could not resource the pass.
fn plan_paths(builder: &mut PlanBuilder<'_>, inputs: &PlanInputs<'_>, pass: u32) {
    let Some((plan, held)) = inputs
        .vectors
        .and_then(|plan| plan.passes.get(pass as usize).map(|held| (plan, held)))
    else {
        builder.defer();
        return;
    };

    builder.round(held.round);
    let (first, count) = builder.stage_vector(vector::instances_of(plan, held));
    if count == 0 {
        builder.defer();
    } else {
        builder.draw(PlannedDraw::Vector {
            target: held.target,
            first,
            count,
        });
    }
}
