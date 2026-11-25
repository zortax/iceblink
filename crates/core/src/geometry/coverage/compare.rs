//! The regions where two outlines can cover a point differently.

use crate::geometry::bounds::DeviceBox;
use crate::geometry::coverage::outline::{Outline, side};

/// Sends every region where `old` and `new` can cover a point differently to `found`.
///
/// Outside the zones of rounded corners, a point's coverage depends only on the two edges of its
/// quadrant, the quarter of the outline around the centre that the point lies in. The regions
/// are:
///
/// 1. A band around every edge that moved or whose ramp changed, as wide as the move and the ramp
///    on either side.
/// 2. The old and the new zone of every rounded corner whose position, radii, exponent or ramp
///    changed.
/// 3. Where the centre moved on one axis, the strip between the two centres, in the rows or
///    columns that hold the zones of rounded corners, since points there change quadrant.
///
/// Regions reach without bound along an edge.
pub(super) fn compare(old: &Outline, new: &Outline, found: &mut impl FnMut(DeviceBox)) {
    let ramp = [old.ramp[0].max(new.ramp[0]), old.ramp[1].max(new.ramp[1])];
    let ramps = old.ramp != new.ramp;

    for edge in 0..4 {
        let (before, after) = (old.edges[edge], new.edges[edge]);
        if before == after && !ramps {
            continue;
        }
        let axis = edge % 2;
        let low = before.min(after) - ramp[axis];
        let high = before.max(after) + ramp[axis];
        found(band(axis, low, high));
    }

    for corner in 0..4 {
        let (before, after) = (old.zone(corner), new.zone(corner));
        if before.is_none() && after.is_none() {
            continue;
        }
        let same = old.corner_point(corner) == new.corner_point(corner)
            && old.radii[corner] == new.radii[corner]
            && old.shapes[corner] == new.shapes[corner]
            && !ramps;
        if !same {
            before.into_iter().chain(after).for_each(&mut *found);
        }
    }

    for axis in 0..2 {
        let (before, after) = (old.centre(axis), new.centre(axis));
        if before == after {
            continue;
        }
        let strip = band(axis, before.min(after), before.max(after));
        for zones in rows(old, new, axis) {
            found(strip.intersect(zones));
        }
    }
}

/// Returns the region between `low` and `high` on axis `axis`, unbounded on the other.
fn band(axis: usize, low: f32, high: f32) -> DeviceBox {
    let open = DeviceBox::EVERYWHERE;
    match axis {
        0 => DeviceBox {
            x0: low,
            x1: high,
            ..open
        },
        _ => DeviceBox {
            y0: low,
            y1: high,
            ..open
        },
    }
}

/// Returns the rows, for `axis` 0, or the columns, for `axis` 1, that hold the zones of the
/// rounded corners of `old` and `new`: one region for the corners on the low side of the other
/// axis and one for those on the high side.
fn rows(old: &Outline, new: &Outline, axis: usize) -> impl Iterator<Item = DeviceBox> {
    let across = 1 - axis;
    let mut low: Option<f32> = None;
    let mut high: Option<f32> = None;

    for outline in [old, new] {
        for corner in 0..4 {
            let Some(zone) = outline.zone(corner) else {
                continue;
            };
            let (right, bottom) = side(corner);
            let far = if across == 0 { right } else { bottom };
            let [start, end] = if across == 0 {
                [zone.x0, zone.x1]
            } else {
                [zone.y0, zone.y1]
            };
            if far {
                high = Some(high.map_or(start, |held| held.min(start)));
            } else {
                low = Some(low.map_or(end, |held| held.max(end)));
            }
        }
    }

    let low = low.map(|end| band(across, f32::NEG_INFINITY, end));
    let high = high.map(|start| band(across, start, f32::INFINITY));
    low.into_iter().chain(high)
}
