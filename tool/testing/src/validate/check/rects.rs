//! Rectangle helpers shared by the checks.

use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};
use iceblink::{DeviceRect, Frame};

/// Returns the rectangle of the output's pixels.
pub(crate) fn output(frame: &Frame) -> DeviceRect {
    let [width, height] = frame.size;
    Rect::new(Point::new(0, 0), Size::new(clamped(width), clamped(height)))
}

fn clamped(length: u32) -> i32 {
    i32::try_from(length).unwrap_or(i32::MAX)
}

/// Reports whether `inner` lies within `outer`.
///
/// An empty rectangle lies within every rectangle.
pub(crate) fn within(inner: DeviceRect, outer: DeviceRect) -> bool {
    inner.is_empty() || outer.contains_rect(inner)
}

/// Reports whether every pixel of `rect` is in `damage`, whichever held rectangles cover it.
pub(crate) fn covered(damage: &DamageSet, rect: DeviceRect) -> bool {
    if damage.is_full() || rect.is_empty() {
        return true;
    }

    // The held rectangles are pairwise disjoint, so their overlaps with `rect` add up.
    let reached: i64 = damage
        .rects()
        .iter()
        .filter_map(|held| held.intersection(rect))
        .map(area)
        .sum();
    reached == area(rect)
}

/// Returns how many pixels `rect` covers.
pub(crate) fn area(rect: DeviceRect) -> i64 {
    i64::from(rect.size.width.max(0)) * i64::from(rect.size.height.max(0))
}

/// Renders a device rectangle as `x,y wxh`.
pub(crate) fn show(rect: DeviceRect) -> String {
    format!(
        "{},{} {}x{}",
        rect.origin.x, rect.origin.y, rect.size.width, rect.size.height
    )
}

/// Reports whether every component is finite.
pub(crate) fn finite(values: &[f32]) -> bool {
    values.iter().all(|value| value.is_finite())
}
