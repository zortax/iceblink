//! The frame's output, damage and shifts.

use iceblink::Frame;
use iceblink::damage::DamageSet;

use crate::capture::text::Writer;
use crate::capture::text::number::float;
use crate::capture::transcript::instance::device;

/// Writes the output the frame is for.
pub(crate) fn output(writer: &mut Writer, frame: &Frame) {
    writer.line(&format!(
        "output {} size={}x{} scale={}{}",
        frame.output.0,
        frame.size[0],
        frame.size[1],
        float(frame.scale),
        if frame.opaque { " opaque" } else { "" }
    ));
}

/// Writes the damage sets and the shifts.
pub(crate) fn damage(writer: &mut Writer, frame: &Frame) {
    writer.line(&format!(
        "damage draw={} present={}",
        set(&frame.damage.draw),
        set(&frame.damage.present)
    ));
    for shift in &frame.shifts {
        writer.line(&format!(
            "shift region={} by=[{}, {}]",
            device(shift.region),
            shift.by[0],
            shift.by[1]
        ));
    }
}

fn set(damage: &DamageSet) -> String {
    if damage.is_full() {
        return "full".to_owned();
    }

    let rects: Vec<String> = damage.rects().iter().map(|rect| device(*rect)).collect();
    format!("[{}]", rects.join(", "))
}
