//! Replaying a frame's batches over one rectangle.
//!
//! A group opens a canvas and draws its content into it, over its written region cut to what the
//! rectangle can reach through the group's filters. Its end runs the filter chain and composites
//! the result into the target beneath. A backdrop captures what is beneath it, filters the copy
//! and composites it back. Batches whose ink misses the rectangle are left out, and an opaque
//! cover that holds the rectangle starts the replay at its quad.

use iceblink::geom::{Device, Rect};
use iceblink::{Batch, BatchKind, OpaqueCover};

use crate::draw::marks::draw_marks;
use crate::draw::sprite::{draw_color_sprites, draw_mono_sprites, draw_subpixel_sprites};
use crate::draw::{draw_external, draw_lines, draw_pass, draw_quads, draw_shadows};
use crate::frame::group::{Open, begin, end};
use crate::frame::{Context, Worker, backdrop};
use crate::target::Surface;

/// Replays every batch of the frame over `rect` into `composed`.
pub(crate) fn replay(
    context: &Context<'_>,
    worker: &mut Worker,
    composed: &mut Surface<'_>,
    rect: Rect<i32, Device>,
) {
    let frame = context.frame;
    let mut stack: Vec<Open> = Vec::new();
    let mut scissor = rect;
    let mut depth = 0usize;

    // The latest cover that holds the rectangle hides the clear and everything before it.
    let hidden = context
        .covers
        .iter()
        .filter(|cover| cover.rect.contains_rect(rect))
        .max_by_key(|cover| (cover.batch, cover.at));
    if hidden.is_none() {
        composed.clear(rect);
    }

    for (index, batch) in frame.batches.iter().enumerate() {
        let Some(batch) = behind(index, batch, hidden) else {
            continue;
        };
        match batch.kind {
            BatchKind::GroupBegin(group) => {
                let open = begin(context, &mut worker.pool, group as usize, scissor, depth);
                depth += 1;
                scissor = open.region;
                stack.push(open);
            }
            BatchKind::GroupEnd(_) => {
                depth = depth.saturating_sub(1);
                let Some(open) = stack.pop() else {
                    continue;
                };
                // A composite into an enclosing group writes that group's whole region, so the
                // enclosing filter reads the same texels in a partial redraw as in a whole one.
                scissor = stack.last().map_or(rect, |outer| outer.region);
                let mut target = current(&mut stack, composed);
                end(context, worker, &mut target, open, scissor);
            }
            BatchKind::Backdrop(index) => {
                let Some(held) = frame.backdrops.get(index as usize) else {
                    continue;
                };
                let mut target = current(&mut stack, composed);
                backdrop::draw(context, worker, &mut target, held, scissor);
            }
            _ => {
                let in_group = stack.iter().any(|open| open.canvas.is_some());
                let mut target = current(&mut stack, composed);
                draw_batch(context, worker, &mut target, &batch, scissor, in_group);
            }
        }
    }

    // Markers pair up in a valid frame, so this closes nothing there.
    while let Some(open) = stack.pop() {
        let enclosing = stack.last().map_or(rect, |outer| outer.region);
        let mut target = current(&mut stack, composed);
        end(context, worker, &mut target, open, enclosing);
    }
}

// The innermost open group's canvas, or the composed target outside every group.
fn current<'s>(stack: &'s mut [Open], composed: &'s mut Surface<'_>) -> Surface<'s> {
    match stack.iter_mut().rev().find_map(|open| open.canvas.as_mut()) {
        Some(canvas) => canvas.surface(),
        None => composed.reborrow(),
    }
}

// The batches an opaque cover leaves to draw: none before its batch, and its own from its quad.
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

// Draws a batch of instances, cut to `scissor`. Subpixel sprites inside a group draw nothing.
fn draw_batch(
    context: &Context<'_>,
    worker: &mut Worker,
    surface: &mut Surface<'_>,
    batch: &Batch,
    scissor: Rect<i32, Device>,
    in_group: bool,
) {
    let frame = context.frame;
    let scene = &context.scene;
    let instances = context.instances;
    let row = &mut worker.row;
    let range =
        |len: usize| (batch.range.start as usize).min(len)..(batch.range.end as usize).min(len);
    match batch.kind {
        BatchKind::External(index) => {
            let Some(external) = frame.externals.get(index as usize) else {
                return;
            };
            // A quad whose texture is not bound draws nothing.
            if let Some(image) = context.externals.get(external) {
                draw_external(scene, surface, scissor, &external.quad, image, row);
                worker.counts.draws += 1;
            }
            return;
        }
        BatchKind::Paths { pass } => {
            let plan = &frame.vector_plan;
            if let (Some(planned), Some(raster)) = (
                plan.passes.get(pass as usize),
                context.rasters.get(pass as usize),
            ) {
                draw_pass(scene, surface, scissor, plan, planned, raster, row);
                worker.counts.draws += 1;
            }
            return;
        }
        BatchKind::Marks => {
            let marks = &instances.marks;
            let range = range(marks.len());
            let first = range.start as u32;
            draw_marks(
                scene,
                surface,
                scissor,
                &marks[range],
                first,
                context.bins,
                row,
            );
            worker.counts.draws += 1;
            return;
        }
        _ => {}
    }
    if !batch.ink.intersects(scissor) {
        worker.counts.culled += 1;
        return;
    }
    worker.counts.draws += 1;
    match batch.kind {
        BatchKind::Quads => draw_quads(
            scene,
            surface,
            scissor,
            &instances.quads[range(instances.quads.len())],
            row,
        ),
        BatchKind::Shadows => draw_shadows(
            scene,
            surface,
            scissor,
            &instances.shadows[range(instances.shadows.len())],
            row,
        ),
        BatchKind::Lines => draw_lines(
            scene,
            surface,
            scissor,
            &instances.lines[range(instances.lines.len())],
            row,
        ),
        BatchKind::Mono { .. } => draw_mono_sprites(
            scene,
            surface,
            scissor,
            &instances.mono[range(instances.mono.len())],
            row,
        ),
        // Per-channel coverage blends correctly only against an opaque destination.
        BatchKind::Subpixel { .. } if in_group => {}
        BatchKind::Subpixel { .. } => draw_subpixel_sprites(
            scene,
            surface,
            scissor,
            &instances.subpixel[range(instances.subpixel.len())],
            row,
            &mut worker.coverage,
        ),
        BatchKind::Color { .. } => draw_color_sprites(
            scene,
            surface,
            scissor,
            &instances.color[range(instances.color.len())],
            row,
        ),
        // Application shaders cannot run here, so a frame for this renderer holds none.
        BatchKind::ShaderQuads { .. } => {}
        _ => {}
    }
}
