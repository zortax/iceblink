//! The resolution a target holds: one texel per device pixel, or one per two by two.

use iceblink::geom::{Device, Point, Rect, Size};

/// The resolution a target holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TargetScale {
    /// One texel per device pixel.
    #[default]
    Full,
    /// One texel per two by two device pixels.
    Half,
}

impl TargetScale {
    /// Returns the texels per device pixel on each axis.
    pub(crate) fn factor(self) -> f32 {
        match self {
            Self::Full => 1.0,
            Self::Half => 0.5,
        }
    }

    /// Returns the texels a target needs to hold `size` device pixels.
    ///
    /// A half-resolution extent is rounded up and is at least one texel.
    pub(crate) fn extent(self, size: Size<i32, Device>) -> Size<i32, Device> {
        match self {
            Self::Full => size.non_negative(),
            Self::Half => Size::new(half_up(size.width).max(1), half_up(size.height).max(1)),
        }
    }

    /// Returns the texel that holds device pixel `pixels` on one axis.
    pub(crate) fn texel(self, pixels: i32) -> i32 {
        match self {
            Self::Full => pixels,
            Self::Half => pixels.div_euclid(2),
        }
    }

    /// Returns the texels that hold `rect`.
    pub(crate) fn texels(self, rect: Rect<i32, Device>) -> Rect<i32, Device> {
        let right = rect.right().max(rect.left());
        let bottom = rect.bottom().max(rect.top());
        Rect::from_corners(
            Point::new(self.texel(rect.left()), self.texel(rect.top())),
            Point::new(self.texel_end(right), self.texel_end(bottom)),
        )
    }

    /// Returns the texels that hold `rect` in a target whose first texel holds device pixel
    /// `origin`.
    ///
    /// A half-resolution target starts on an even device pixel.
    pub(crate) fn texels_at(
        self,
        rect: Rect<i32, Device>,
        origin: Point<i32, Device>,
    ) -> Rect<i32, Device> {
        debug_assert!(
            self == Self::Full || (origin.x % 2 == 0 && origin.y % 2 == 0),
            "a half-resolution target starts on an even device pixel, not at {origin:?}"
        );
        self.texels(rect.translate(Point::ORIGIN.offset_from(origin)))
    }

    /// Returns the end of the texels that hold the device pixels before `pixels` on one axis.
    fn texel_end(self, pixels: i32) -> i32 {
        match self {
            Self::Full => pixels,
            Self::Half => (pixels + 1).div_euclid(2),
        }
    }
}

/// Returns half of `length` rounded up, and zero for a negative length.
fn half_up(length: i32) -> i32 {
    (length.max(0) + 1) / 2
}

#[cfg(test)]
mod tests {
    use super::TargetScale;
    use iceblink::geom::{Point, Rect, Size};

    #[test]
    fn halving_an_odd_extent_rounds_up_so_the_last_half_pixel_has_a_texel() {
        assert_eq!(TargetScale::Half.extent(Size::new(9, 1)), Size::new(5, 1));
        assert_eq!(TargetScale::Full.extent(Size::new(9, 1)), Size::new(9, 1));
    }

    #[test]
    fn an_empty_extent_still_allocates_a_texel_because_a_texture_cannot_be_empty() {
        assert_eq!(TargetScale::Half.extent(Size::new(0, 0)), Size::new(1, 1));
    }

    #[test]
    fn the_factor_and_the_extent_agree() {
        for scale in [TargetScale::Full, TargetScale::Half] {
            let extent = scale.extent(Size::new(64, 64));
            assert_eq!(extent.width as f32, 64.0 * scale.factor());
        }
    }

    #[test]
    fn a_rectangle_ending_on_an_odd_pixel_keeps_the_texel_holding_that_pixel() {
        let rect = Rect::from_corners(Point::new(3, 1), Point::new(11, 9));
        assert_eq!(
            TargetScale::Half.texels(rect),
            Rect::from_corners(Point::new(1, 0), Point::new(6, 5))
        );
        assert_eq!(TargetScale::Full.texels(rect), rect);
    }

    #[test]
    fn a_rectangle_is_measured_from_the_target_origin() {
        let rect = Rect::from_corners(Point::new(3, 1), Point::new(11, 9));
        assert_eq!(TargetScale::Full.texels_at(rect, Point::new(0, 0)), rect);
        assert_eq!(
            TargetScale::Full.texels_at(rect, Point::new(2, -4)),
            Rect::from_corners(Point::new(1, 5), Point::new(9, 13))
        );
        assert_eq!(
            TargetScale::Half.texels_at(rect, Point::new(2, -4)),
            Rect::from_corners(Point::new(0, 2), Point::new(5, 7))
        );
    }
}
