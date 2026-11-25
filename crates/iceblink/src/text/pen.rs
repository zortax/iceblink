//! The split of a glyph's horizontal position into a pixel and a phase.

use crate::geom::{Point, Rect, Size};
use crate::geometry::DeviceRect;
use crate::text::GlyphPlacement;

/// How many phases a pixel is divided into.
pub const PHASES: u8 = 4;

/// A glyph's absolute horizontal device position, split into the whole pixel its bitmap is drawn
/// from and the phase it is rasterised at.
///
/// The position is quantised to a quarter pixel first, and both halves are read from the
/// quantised value: `pen + phase / 4` is the position rounded to the nearest quarter. A fraction
/// that rounds up to the next pixel moves `pen`, and the phase is zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenPosition {
    /// The whole pixel the bitmap's left edge is measured from.
    pen: f32,
    /// The phase, in quarter pixels.
    phase: u8,
}

impl PenPosition {
    /// Splits the absolute horizontal device position `x` of a glyph's origin.
    pub fn of(x: f32) -> Self {
        let steps = f32::from(PHASES);
        let quantised = (x * steps).round() / steps;
        let pen = quantised.floor();
        let step = ((quantised - pen) * steps).round() as u8;
        Self {
            pen,
            phase: step % PHASES,
        }
    }

    /// Returns the whole pixel the bitmap's left edge is measured from.
    pub fn pen(self) -> f32 {
        self.pen
    }

    /// Returns the phase the glyph is rasterised at, in quarter pixels from 0 to 3.
    pub fn phase(self) -> u8 {
        self.phase
    }

    /// Returns the quantised position, `pen + phase / 4`.
    pub fn quantised(self) -> f32 {
        self.pen + f32::from(self.phase) / f32::from(PHASES)
    }

    /// Returns the device position of the top-left corner of a bitmap rasterised at this phase,
    /// for a glyph whose origin lies on the absolute device baseline `baseline`.
    ///
    /// The left edge is `pen + placement.left` and the top edge is
    /// `round(baseline) - placement.top`, so the bitmap lands on whole device pixels.
    pub fn bitmap_origin(self, baseline: f32, placement: GlyphPlacement) -> [f32; 2] {
        [
            self.pen + placement.left as f32,
            baseline.round() - placement.top as f32,
        ]
    }

    /// Returns the device pixels the bitmap of an unhinted glyph covers.
    ///
    /// The glyph's origin lies at this position on the absolute device baseline `baseline`.
    /// `bounds` is the bounding box of the glyph's outline points, control points included, as
    /// `[x0, y0, x1, y1]` in device pixels with y down and the origin at zero. The outline is the
    /// one `GlyphRaster::outline` writes at the device size, slanted by the run's skew. An
    /// emboldening above zero moves every edge of `bounds` outwards by it. A subpixel bitmap
    /// extends a third of a pixel further on each side.
    ///
    /// Returns `None` when the box covers no pixel.
    pub fn bitmap_box(self, baseline: f32, bounds: [f32; 4], subpixel: bool) -> Option<DeviceRect> {
        let [x0, y0, x1, y1] = bounds;
        let phase = f32::from(self.phase) / f32::from(PHASES);
        // The sums keep the rasteriser's order, so an edge that lands on a whole pixel rounds the
        // same way.
        let (mut left, mut right) = (x0 + phase, x1 + phase);
        if subpixel {
            left += -1.0 / 3.0;
            right += 1.0 / 3.0;
        }
        let pen = self.pen as i32;
        let baseline = baseline.round() as i32;
        let x = pen + left.floor() as i32;
        let y = baseline + y0.floor() as i32;
        let width = pen + right.ceil() as i32 - x;
        let height = baseline + y1.ceil() as i32 - y;
        (width > 0 && height > 0).then(|| Rect::new(Point::new(x, y), Size::new(width, height)))
    }
}
