//! Placements: shapes rasterised at quarter-pixel phases into atlas cells, drawn on whole device
//! pixels.

use iceblink::geom::{Device, Point, Rect, Size};
use iceblink::{FrameMarkPayload, FrameMarks, MarkCell};

use crate::draw::marks::mapping::Mapping;
use crate::draw::marks::paint::colour;
use crate::draw::{Scene, paint_area};
use crate::math::Floor as _;
use crate::pages::Page;
use crate::tables::Painter;
use crate::target::Surface;

/// A placement's cell on the device.
#[derive(Clone, Copy, Debug)]
pub(super) struct Placed {
    /// The device pixels the cell covers.
    pub(super) pixels: Rect<i32, Device>,
    /// The cell's first texel on the page.
    texel: [i32; 2],
}

impl Placed {
    /// Returns the coverage the cell holds at device pixel `(x, y)`, or nothing outside it.
    pub(super) fn coverage(&self, page: &Page, x: i32, y: i32) -> f32 {
        if !self.pixels.contains(Point::new(x, y)) {
            return 0.0;
        }
        page.load(
            0,
            self.texel[0] + x - self.pixels.left(),
            self.texel[1] + y - self.pixels.top(),
        )[0]
    }
}

/// Returns the cell of every placement `item` draws, at the phase of its anchor.
///
/// A placement whose shape has no cells, or whose cell is empty, is left out.
pub(super) fn placed(
    item: &FrameMarks,
    payload: &FrameMarkPayload,
    mapping: &Mapping<'_>,
) -> impl Iterator<Item = Placed> {
    let marks = item.marks;
    let mapping = *mapping;
    let data = payload.payload.clone();
    let cells = payload.cells.clone();
    (0..marks.placements as usize).filter_map(move |index| {
        let placement = data.placements.get(marks.first as usize + index)?;
        let anchor = mapping.map([placement.x, placement.y]);
        let device = mapping.space.device_unchecked(anchor);
        let quantised = [
            (4.0 * device[0] + 0.5).floored(),
            (4.0 * device[1] + 0.5).floored(),
        ];
        let phase = quantised.map(|value| value - 4.0 * (value / 4.0).floored());
        let pixel = [
            ((quantised[0] - phase[0]) / 4.0) as i32,
            ((quantised[1] - phase[1]) / 4.0) as i32,
        ];
        let index = placement.shape as usize * 16 + phase[0] as usize + 4 * phase[1] as usize;
        let cell: &MarkCell = cells.get(index)?;
        let size = Size::new(cell.rect[2] as i32, cell.rect[3] as i32);
        if size.is_empty() {
            return None;
        }
        Some(Placed {
            pixels: Rect::new(
                Point::new(pixel[0] + cell.offset[0], pixel[1] + cell.offset[1]),
                size,
            ),
            texel: [cell.rect[0] as i32, cell.rect[1] as i32],
        })
    })
}

/// Paints every placement of `item` on its own.
pub(super) fn paint(
    scene: &Scene<'_>,
    surface: &mut Surface<'_>,
    within: Rect<i32, Device>,
    item: &FrameMarks,
    payload: &FrameMarkPayload,
    row: &mut Vec<[f32; 4]>,
) {
    if item.marks.placements == 0 {
        return;
    }
    let mapping = &Mapping::of(&item.marks, scene.space(item.marks.space.0));
    let clip = scene.clip(item.marks.clip.0);
    let paint = &Painter::of(scene.paints, item.marks.paint);
    let Some(page) = scene.pages.get(item.page) else {
        return;
    };
    for cell in placed(item, payload, mapping) {
        let Some(area) = cell.pixels.intersection(within) else {
            continue;
        };
        paint_area(surface, area, row, |device| {
            let x = device[0].floored() as i32;
            let y = device[1].floored() as i32;
            let covered = cell.coverage(page, x, y);
            let local = mapping.space.local(device).unwrap_or(device);
            colour(
                scene,
                clip,
                paint,
                item.marks.paint_origin,
                device,
                local,
                covered,
            )
        });
    }
}
