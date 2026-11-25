//! The device rectangles a frame's entries ink.

use iceblink::geom::{Point, Rect};
use iceblink::{DeviceRect, Matrix4};

use crate::conformance::draw::builder::FrameBuilder;

impl FrameBuilder {
    /// Returns the device rectangle `bounds` inks in space slot `space`, grown by 2 pixels.
    ///
    /// The ink is cut to the output unless a group is open. A transform that is not finite inks
    /// the whole output.
    pub(super) fn ink(&self, bounds: [f32; 4], space: u32) -> DeviceRect {
        let matrix = self
            .spaces
            .get(space as usize)
            .copied()
            .unwrap_or(Matrix4::IDENTITY);
        let [x, y, w, h] = bounds;
        let corners = [(x, y), (x + w, y), (x, y + h), (x + w, y + h)];
        let mut low = (f32::INFINITY, f32::INFINITY);
        let mut high = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for (cx, cy) in corners {
            let [px, py, _] = matrix.transform_point(cx, cy, 0.0);
            low = (low.0.min(px), low.1.min(py));
            high = (high.0.max(px), high.1.max(py));
        }
        if !(low.0.is_finite() && low.1.is_finite() && high.0.is_finite() && high.1.is_finite()) {
            return self.output;
        }

        let ink = Rect::from_corners(
            Point::new(low.0.floor() as i32 - 2, low.1.floor() as i32 - 2),
            Point::new(high.0.ceil() as i32 + 2, high.1.ceil() as i32 + 2),
        );
        // A group's target reaches past the output, over the group's source.
        if !self.open.is_empty() {
            return ink;
        }
        ink.intersection(self.output).unwrap_or(Rect::ZERO)
    }
}

/// Cuts a device rectangle to the output. A rectangle wholly outside it becomes empty.
pub(super) fn cut([x, y, w, h]: [f32; 4], output: DeviceRect) -> [f32; 4] {
    let width = output.size.width as f32;
    let height = output.size.height as f32;
    let left = x.clamp(0.0, width);
    let top = y.clamp(0.0, height);
    let right = (x + w).clamp(left, width);
    let bottom = (y + h).clamp(top, height);
    [left, top, right - left, bottom - top]
}

/// Returns `bounds` rounded outwards to whole pixels and cut to the output.
pub(super) fn rounded(bounds: [f32; 4], output: DeviceRect) -> DeviceRect {
    let [x, y, w, h] = bounds;
    Rect::from_corners(
        Point::new(x.floor() as i32, y.floor() as i32),
        Point::new((x + w).ceil() as i32, (y + h).ceil() as i32),
    )
    .intersection(output)
    .unwrap_or(Rect::ZERO)
}

/// Returns the pass region of `ink`: aligned outwards to multiples of 16 and cut to the output.
pub(super) fn aligned(ink: DeviceRect, output: DeviceRect) -> DeviceRect {
    let left = ink.origin.x.div_euclid(16) * 16;
    let top = ink.origin.y.div_euclid(16) * 16;
    let right = (ink.origin.x + ink.size.width + 15).div_euclid(16) * 16;
    let bottom = (ink.origin.y + ink.size.height + 15).div_euclid(16) * 16;
    Rect::from_corners(Point::new(left, top), Point::new(right, bottom))
        .intersection(output)
        .unwrap_or(Rect::ZERO)
}
