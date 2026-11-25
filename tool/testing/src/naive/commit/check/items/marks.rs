//! Marks items and the parts of their payloads they draw.

use iceblink::{MarkPayload, Marks, RejectReason};

use crate::naive::commit::check::finite::{numbers, path};
use crate::naive::commit::check::items::range;
use crate::naive::commit::check::items::scope::Scope;
use crate::naive::commit::check::nodes::valid;

pub(super) fn check(scope: &Scope<'_>, marks: &Marks) -> Result<(), RejectReason> {
    numbers(&marks.bounds)?;
    numbers(&marks.paint_origin)?;
    numbers(&marks.origin)?;
    numbers(&marks.axes)?;
    numbers(&[marks.half_width])?;
    valid(marks.flags.is_valid())?;
    scope.paint(marks.paint)?;
    scope.clip(marks.clip)?;
    scope.space(marks.space)?;

    let payload = scope
        .chunk
        .payloads
        .get(marks.payload as usize)
        .ok_or(RejectReason::InvalidItem)?;
    drawn(marks, payload)
}

/// Checks that the prims `marks` draws lie inside the payload, are finite, and refer to shapes
/// the payload holds.
fn drawn(marks: &Marks, payload: &MarkPayload) -> Result<(), RejectReason> {
    let counts = [marks.discs, marks.boxes, marks.vertices, marks.placements];
    let kinds = counts.iter().filter(|count| **count > 0).count();
    let first = if kinds == 1 { marks.first } else { 0 };
    valid(kinds == 1 || marks.first == 0)?;

    let discs = payload
        .discs
        .get(range(first, marks.discs)?)
        .ok_or(RejectReason::InvalidItem)?;
    discs.iter().try_for_each(|disc| numbers(disc))?;

    let boxes = payload
        .boxes
        .get(range(first, marks.boxes)?)
        .ok_or(RejectReason::InvalidItem)?;
    boxes.iter().try_for_each(|mark| {
        numbers(&mark.rect)?;
        numbers(&mark.radii)?;
        numbers(&mark.shape)
    })?;

    let vertices = payload
        .vertices
        .get(range(first, marks.vertices)?)
        .ok_or(RejectReason::InvalidItem)?;
    // A vertex with a NaN `x` separates runs.
    vertices
        .iter()
        .filter(|vertex| !vertex[0].is_nan())
        .try_for_each(|vertex| numbers(vertex))?;

    let placements = payload
        .placements
        .get(range(first, marks.placements)?)
        .ok_or(RejectReason::InvalidItem)?;
    for placement in placements {
        numbers(&[placement.x, placement.y])?;
        valid((placement.shape as usize) < payload.shapes.len())?;
    }

    if !placements.is_empty() {
        payload.shapes.iter().try_for_each(|shape| path(shape))?;
    }

    Ok(())
}
