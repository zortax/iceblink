//! Marks and their payloads.

use iceblink::{FrameMarkPayload, FrameMarks, PageFormat};

use crate::validate::Rule;
use crate::validate::check::Inspect;
use crate::validate::check::resolve::Sources;

/// Checks every resolved marks instance and the payload it reads.
pub(super) fn check(inspect: &mut Inspect<'_>, sources: &Sources<'_, FrameMarks>) {
    let transient = &inspect.frame.lanes.mark_payloads;
    for (position, resolved) in sources.iter().enumerate() {
        let Some(resolved) = resolved else {
            continue;
        };
        let payloads = resolved
            .chunk
            .map_or(transient.as_slice(), |chunk| chunk.mark_payloads.as_slice());
        one(inspect, position, &resolved.instance, payloads);
    }
}

fn one(
    inspect: &mut Inspect<'_>,
    position: usize,
    marks: &FrameMarks,
    payloads: &[FrameMarkPayload],
) {
    let at = || format!("marks source {position}");
    let item = &marks.marks;
    inspect.clip(item.clip.0, at);
    inspect.space(item.space.0, at);
    inspect.paint(item.paint, at);
    if !item.flags.is_valid() {
        inspect.push(
            Rule::Values,
            format!("{} has mark flags {:#x}", at(), item.flags.0),
        );
    }
    let Some(payload) = payloads.get(item.payload as usize) else {
        inspect.push(
            Rule::MarkPayloads,
            format!(
                "{} names payload {} of {}",
                at(),
                item.payload,
                payloads.len()
            ),
        );
        return;
    };
    let shapes = payload.payload.shapes.len();
    if payload.cells.len() != shapes * 16 {
        inspect.push(
            Rule::MarkPayloads,
            format!(
                "{} has {} cells for {shapes} shapes",
                at(),
                payload.cells.len()
            ),
        );
    }

    let lists = [
        ("discs", item.discs, payload.payload.discs.len()),
        ("boxes", item.boxes, payload.payload.boxes.len()),
        ("vertices", item.vertices, payload.payload.vertices.len()),
        (
            "placements",
            item.placements,
            payload.payload.placements.len(),
        ),
    ];
    let single = lists.iter().filter(|(_, count, _)| *count > 0).count() == 1;
    if !single && item.first != 0 {
        inspect.push(
            Rule::MarkPayloads,
            format!(
                "{} starts at {} while drawing several prim kinds",
                at(),
                item.first
            ),
        );
    }
    let first = if single { item.first as usize } else { 0 };
    for (name, count, len) in lists {
        if count > 0 && first + count as usize > len {
            inspect.push(
                Rule::MarkPayloads,
                format!("{} reads {count} {name} from {first} of {len}", at()),
            );
            return;
        }
    }

    let placed = if item.placements > 0 {
        &payload.payload.placements[first..first + item.placements as usize]
    } else {
        &[]
    };
    // The page is read only for a placement whose shape has a cell to draw.
    let reads_page = placed.iter().any(|placement| {
        let start = placement.shape as usize * 16;
        payload.cells.get(start..start + 16).is_some_and(|cells| {
            cells
                .iter()
                .any(|cell| cell.rect[2] > 0 && cell.rect[3] > 0)
        })
    });
    if reads_page {
        inspect.page(marks.page, PageFormat::A8, at);
    }
    if let Some(placement) = placed
        .iter()
        .find(|placement| placement.shape as usize >= shapes)
    {
        inspect.push(
            Rule::MarkPayloads,
            format!("{} places shape {} of {shapes}", at(), placement.shape),
        );
    }
}
