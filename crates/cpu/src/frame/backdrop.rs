//! Backdrops: what is drawn beneath a rectangle, captured, filtered and composited back.

use iceblink::geom::{Device, Rect};
use iceblink::{BatchKind, BlendMode, Frame, FrameBackdrop};

use crate::frame::filter::{self, Chain, Composite, EdgeMode};
use crate::frame::group::{MAX_REACH, RESAMPLE_SLACK, STEP_SLACK};
use crate::frame::rects::{reads_only_what_it_writes, rounded_out};
use crate::frame::{Context, Worker};
use crate::target::{Canvas, Surface};

/// Draws `backdrop` into `beneath`, the target open at its batch, cut to `scissor`.
///
/// A backdrop that writes nothing inside the scissor reads nothing.
pub(crate) fn draw(
    context: &Context<'_>,
    worker: &mut Worker,
    beneath: &mut Surface<'_>,
    backdrop: &FrameBackdrop,
    scissor: Rect<i32, Device>,
) {
    let written = rounded_out(backdrop.bounds);
    let Some(bounds) = written
        .intersection(scissor)
        .and_then(|cut| cut.intersection(context.used))
    else {
        return;
    };
    let Some(source) = rounded_out(backdrop.source).intersection(context.used) else {
        return;
    };
    let captured = capture(worker, beneath, source);

    let chain = Chain::of(&backdrop.filters);
    let filtered = filter::run(
        &mut worker.pool,
        &chain,
        captured,
        source,
        source,
        EdgeMode::Duplicate,
    );
    let how = Composite {
        bounds,
        clip: backdrop.backdrop.clip.0,
        opacity: 1.0,
        blend: BlendMode::NORMAL,
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

// Returns a copy of what `beneath` holds over `source`.
fn capture(worker: &mut Worker, beneath: &Surface<'_>, source: Rect<i32, Device>) -> Canvas {
    let mut captured = worker.pool.acquire(source, beneath.precision(), false);
    let texture = beneath.texture();
    let mut surface = captured.surface();
    if let Some(held) = source.intersection(beneath.extent()) {
        let row = &mut worker.row;
        for y in held.top()..held.bottom() {
            row.clear();
            row.extend((held.left()..held.right()).map(|x| texture.at_pixel(x, y)));
            surface.replace_row(held.left(), y, row);
        }
    }
    captured
}

/// Returns how far each group's region must extend so every backdrop inside it reads what was
/// drawn there, by group.
pub(crate) fn nested_reach(frame: &Frame) -> Vec<i32> {
    let mut reach = vec![0; frame.groups.len()];
    if frame.backdrops.is_empty() {
        return reach;
    }
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
