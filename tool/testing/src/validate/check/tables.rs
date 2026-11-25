//! The values a frame writes into its tables.

use iceblink::{Color, Frame, FramePaint, TableDelta};

use crate::validate::Rule;
use crate::validate::check::Report;
use crate::validate::check::rects::finite;

/// How far a premultiplied colour channel may exceed its alpha before it counts as straight.
const PREMULTIPLIED_SLACK: f32 = 1.0e-4;

/// Checks every matrix and paint the frame's deltas write.
pub(crate) fn check(frame: &Frame, report: &mut Report) {
    for (slot, matrix) in written(&frame.tables.spaces) {
        if !finite(matrix.columns.as_flattened()) {
            report.push(
                Rule::Values,
                format!("space slot {slot} has a non-finite matrix"),
            );
        }
    }

    for (slot, paint) in written(&frame.tables.paints) {
        paint_value(slot, paint, report);
    }
}

/// Every slot a delta writes, with its value, in the order they apply.
fn written<T>(delta: &TableDelta<T>) -> impl Iterator<Item = (u32, &T)> {
    let all = delta
        .all
        .iter()
        .enumerate()
        .filter(|_| delta.replace)
        .map(|(slot, value)| (slot as u32, value));
    all.chain(delta.changed.iter().map(|(slot, value)| (*slot, value)))
}

fn paint_value(slot: u32, paint: &FramePaint, report: &mut Report) {
    match paint {
        FramePaint::None => {}
        FramePaint::Solid(color) => color_value(slot, *color, report),
        FramePaint::Sides(colors) => {
            for color in colors {
                color_value(slot, *color, report);
            }
        }
        FramePaint::Gradient(gradient) => {
            if !gradient.encoding.is_valid() {
                report.push(
                    Rule::Values,
                    format!(
                        "paint slot {slot} has stop encoding {}",
                        gradient.encoding.0
                    ),
                );
            }
            let ordered = gradient
                .stops
                .windows(2)
                .all(|pair| pair[0].offset <= pair[1].offset);
            if !ordered {
                report.push(
                    Rule::Values,
                    format!("paint slot {slot} has stops out of offset order"),
                );
            }
        }
    }
}

fn color_value(slot: u32, color: Color, report: &mut Report) {
    let [red, green, blue, alpha] = color.0;
    let in_range = color.0.iter().all(|channel| (0.0..=1.0).contains(channel));
    let premultiplied = [red, green, blue]
        .iter()
        .all(|channel| *channel <= alpha + PREMULTIPLIED_SLACK);
    if !in_range || !premultiplied {
        report.push(
            Rule::Values,
            format!(
                "paint slot {slot} has colour {:?}, which is not premultiplied",
                color.0
            ),
        );
    }
}
