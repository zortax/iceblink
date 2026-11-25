//! Planning marks: their coverage bins on mark pages, and their draws in the replay.

use core::ops::Range;

use iceblink::geom::{Device, Point, Rect, Size};
use iceblink::{FrameMarks, MarkBin, MarkFlags, MarkPlan};

use crate::frame::build::PlanBuilder;
use crate::frame::plan::PlanInputs;
use crate::frame::segment::PlannedDraw;
use crate::frame::target::TargetRef;
use crate::pipeline::marks::{MarkDraw, MarkKind};

/// Plans one pass per mark page, which draws the coverage of every item binned on that page.
pub(super) fn plan_mark_pages(
    builder: &mut PlanBuilder<'_>,
    inputs: &PlanInputs<'_>,
    plan: &MarkPlan,
) {
    if plan.pages == 0 {
        return;
    }
    let page = page_rect(plan);
    builder.set_mark_extent(page.size);

    for index in 0..plan.pages {
        builder.begin_pass(TargetRef::MarksPage(index), page);
        for bin in plan.bins.iter().filter(|bin| bin.page == index) {
            let position = bin.item as usize;
            let Some(item) = (inputs.marks)(position) else {
                continue;
            };
            let block = stage(builder, inputs, position, &item, Some(bin));
            draw_mark_kinds(builder, &item, position, block, true);
        }
    }
    builder.end_pass();
}

/// Plans the marks items `range` into `target`.
///
/// A union item composites its coverage bin. An item binned on the spare page has its coverage
/// drawn right before its composite. Every other item draws its prims directly.
pub(super) fn plan_marks(
    builder: &mut PlanBuilder<'_>,
    inputs: &PlanInputs<'_>,
    range: Range<u32>,
    target: TargetRef,
    scissor: Rect<i32, Device>,
) {
    let plan = &inputs.frame.mark_plan;
    for position in range {
        let position = position as usize;
        let Some(item) = (inputs.marks)(position) else {
            continue;
        };
        if !item.marks.flags.contains(MarkFlags::UNION) {
            let block = stage(builder, inputs, position, &item, None);
            draw_mark_kinds(builder, &item, position, block, false);
            continue;
        }

        if let Some(bin) = plan.bin(position as u32) {
            let block = stage(builder, inputs, position, &item, Some(bin));
            builder.draw(PlannedDraw::MarksComposite { block });
        } else if let Some(bin) = plan.overflow_bin(position as u32) {
            // The spare page holds one item at a time, written right before its composite.
            let block = stage(builder, inputs, position, &item, Some(bin));
            builder.begin_pass(TargetRef::MarksPage(bin.page), page_rect(plan));
            draw_mark_kinds(builder, &item, position, block, true);
            builder.begin_pass(target, scissor);
            builder.draw(PlannedDraw::MarksComposite { block });
        }
        // With no bin, the item's ink misses the damage.
    }
}

/// Returns the whole of one mark page.
fn page_rect(plan: &MarkPlan) -> Rect<i32, Device> {
    let extent = Size::new(plan.extent[0] as i32, plan.extent[1] as i32);
    Rect::new(Point::new(0, 0), extent)
}

/// Stages the mark draw block of the item at `position`, once per frame, and returns its offset.
///
/// A binned item's block places its coverage in `bin`.
fn stage(
    builder: &mut PlanBuilder<'_>,
    inputs: &PlanInputs<'_>,
    position: usize,
    item: &FrameMarks,
    bin: Option<&MarkBin>,
) -> u32 {
    let cells = if item.marks.placements > 0 {
        (inputs.cells)(position)
    } else {
        0
    };
    builder.stage_mark_draw(position, || {
        let mut draw = match bin {
            Some(bin) => MarkDraw::binned(
                position as u32,
                bin.page,
                bin.region,
                Point::new(bin.at[0] as i32, bin.at[1] as i32),
            ),
            None => MarkDraw::direct(position as u32),
        };
        draw.cells[0] = cells;
        draw
    })
}

/// Plans one draw per kind of prim the item at `position` holds.
///
/// A `coverage` draw adds to a coverage bin and covers only the kinds that are binned.
fn draw_mark_kinds(
    builder: &mut PlanBuilder<'_>,
    item: &FrameMarks,
    position: usize,
    block: u32,
    coverage: bool,
) {
    let kinds = if coverage {
        MarkKind::BINNED
    } else {
        MarkKind::ALL
    };
    for kind in kinds {
        let instances = mark_instances(item, kind);
        if !instances.is_empty() {
            builder.draw(PlannedDraw::Marks {
                kind,
                coverage,
                position,
                block,
                instances: [instances.start, instances.end],
                page: (kind == MarkKind::Glyph).then_some(item.page),
            });
        }
    }
}

/// Returns the prims of one payload kind an item draws, from the start of the kind's payload.
fn mark_instances(item: &FrameMarks, kind: MarkKind) -> Range<u32> {
    let marks = &item.marks;
    let first = marks.first;
    let (start, end) = match kind {
        MarkKind::Disc => (first, first.saturating_add(marks.discs)),
        MarkKind::Box => (first, first.saturating_add(marks.boxes)),
        // The segments of a polyline lie between its separators.
        MarkKind::Polyline if marks.vertices >= 3 => (
            first.saturating_add(1),
            first.saturating_add(marks.vertices - 2),
        ),
        MarkKind::Polyline => (0, 0),
        MarkKind::Glyph => (first, first.saturating_add(marks.placements)),
    };
    start..end.max(start)
}
