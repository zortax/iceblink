//! Marks: discs, boxes, polylines and placed shapes from a shared payload.
//!
//! A payload maps to the item's local space by the item's axes and origin. Distances are measured
//! in a distance space: payload units without `SCREEN`, so every length scales with the axes, and
//! local units with it. An item without `UNION` paints each prim on its own. A union item paints
//! through its bin, which holds its prims' coverage of each quarter of every pixel.

mod bins;
mod glyph;
mod mapping;
mod paint;
mod prims;

use iceblink::geom::{Device, Rect};
use iceblink::{FrameMarkPayload, FrameMarks, MarkFlags};

use crate::draw::Scene;
use crate::target::Surface;

pub(crate) use self::bins::Bins;

/// A marks instance with the payload it draws.
#[derive(Clone, Debug)]
pub(crate) struct MarkInstance {
    pub(crate) item: FrameMarks,
    pub(crate) payload: Option<FrameMarkPayload>,
}

/// Draws `marks` into `surface`, cut to `scissor`.
///
/// `first` is the position of the first instance in the marks lane. A union instance paints
/// through its bin in `bins`, and draws nothing without one.
pub(crate) fn draw_marks(
    scene: &Scene<'_>,
    surface: &mut Surface<'_>,
    scissor: Rect<i32, Device>,
    marks: &[MarkInstance],
    first: u32,
    bins: &Bins,
    row: &mut Vec<[f32; 4]>,
) {
    let Some(within) = scissor.intersection(surface.extent()) else {
        return;
    };
    for (offset, instance) in marks.iter().enumerate() {
        let Some(payload) = &instance.payload else {
            continue;
        };
        if instance.item.marks.flags.contains(MarkFlags::UNION) {
            if let Some(bin) = bins.get(first + offset as u32) {
                paint::composite(scene, surface, within, &instance.item, bin, row);
            }
            continue;
        }
        paint::prims(scene, surface, within, &instance.item, payload, row);
    }
}
