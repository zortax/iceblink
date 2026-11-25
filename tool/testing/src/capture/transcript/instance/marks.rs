//! Marks instances, rendered as the counts, flags and placement of their payload.
//!
//! The payload itself is left out.

use iceblink::{FrameMarks, MarkFlags};

use crate::capture::mirror::Mirror;
use crate::capture::text::number::{all_zero, float, list, rect};
use crate::capture::transcript::instance::boxes::origin;
use crate::capture::transcript::{paint, space};

pub(crate) fn marks(mirror: &Mirror, framed: &FrameMarks) -> String {
    let marks = &framed.marks;
    let mut line = format!(
        "marks bounds={} paint={} discs={} boxes={} vertices={} placements={}",
        rect(marks.bounds),
        paint::reference(mirror, marks.paint),
        marks.discs,
        marks.boxes,
        marks.vertices,
        marks.placements
    );
    if marks.first != 0 {
        line.push_str(&format!(" first={}", marks.first));
    }
    for (flag, name) in [
        (MarkFlags::UNION, "union"),
        (MarkFlags::SCREEN, "screen"),
        (MarkFlags::SQUARE_DISCS, "square_discs"),
    ] {
        if marks.flags.contains(flag) {
            line.push(' ');
            line.push_str(name);
        }
    }
    if marks.axes != [1.0, 0.0, 0.0, 1.0] {
        line.push_str(&format!(" axes={}", list(&marks.axes)));
    }
    if marks.vertices > 0 {
        line.push_str(&format!(
            " half_width={} caps={}/{}",
            float(marks.half_width),
            cap(marks.flags.start_cap()),
            cap(marks.flags.end_cap())
        ));
    }
    if !all_zero(&marks.origin) {
        line.push_str(&format!(" origin={}", list(&marks.origin)));
    }
    origin(&mut line, marks.paint_origin);
    if marks.placements > 0 {
        line.push_str(&format!(" page={}", framed.page));
    }
    line.push_str(&space::suffix(mirror, marks.clip.0, marks.space.0));
    line
}

fn cap(cap: u32) -> String {
    match cap {
        MarkFlags::CAP_BUTT => "butt".to_owned(),
        MarkFlags::CAP_SQUARE => "square".to_owned(),
        MarkFlags::CAP_ROUND => "round".to_owned(),
        other => format!("<cap {other}>"),
    }
}
