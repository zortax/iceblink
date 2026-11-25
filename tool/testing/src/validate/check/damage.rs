//! The damage sets and shifts of a frame.

use iceblink::Frame;
use iceblink::damage::DamageSet;

use crate::validate::Rule;
use crate::validate::check::Report;
use crate::validate::check::rects::{covered, output, show, within};

/// Checks that damage and shifts lie in the output and that shifts expose only drawn pixels.
pub(crate) fn check(frame: &Frame, report: &mut Report) {
    rects_inside(frame, &frame.damage.draw, "draw", report);
    rects_inside(frame, &frame.damage.present, "present", report);

    let output = output(frame);
    for (index, shift) in frame.shifts.iter().enumerate() {
        if !within(shift.region, output) {
            report.push(
                Rule::Damage,
                format!(
                    "shift {index} region {} leaves the output",
                    show(shift.region)
                ),
            );
        }
        shift.for_each_exposed(|band| {
            if !covered(&frame.damage.draw, band) {
                report.push(
                    Rule::Damage,
                    format!(
                        "shift {index} exposes {} outside the draw damage",
                        show(band)
                    ),
                );
            }
        });
    }
}

fn rects_inside(frame: &Frame, damage: &DamageSet, name: &str, report: &mut Report) {
    let output = output(frame);
    for rect in damage.rects() {
        if !within(*rect, output) {
            report.push(
                Rule::Damage,
                format!("{name} damage {} leaves the output", show(*rect)),
            );
        }
    }
}
