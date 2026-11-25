//! The kinds of mark draws and the block each draw binds.

use bytemuck::{Pod, Zeroable};
use iceblink::geom::{Device, Point, Rect};

/// The shape a mark draw draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MarkKind {
    Disc,
    Box,
    Polyline,
    Glyph,
}

impl MarkKind {
    pub(crate) const ALL: [Self; 4] = [Self::Disc, Self::Box, Self::Polyline, Self::Glyph];

    /// The kinds in the order they are drawn into a bin page.
    pub(crate) const BINNED: [Self; 4] = [Self::Polyline, Self::Disc, Self::Box, Self::Glyph];

    /// Returns the index of the kind's payload lane.
    pub(crate) fn lane(self) -> usize {
        self as usize
    }
}

/// The record of one mark draw: the item it draws, and where in a bin page it draws it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct MarkDraw {
    pub(crate) position: u32,
    pub(crate) page: u32,
    pub(crate) shift: [f32; 2],
    pub(crate) region: [f32; 4],
    pub(crate) cells: [u32; 4],
}

impl MarkDraw {
    /// Returns the record of a draw of item `position` straight into its target.
    pub(crate) fn direct(position: u32) -> Self {
        Self {
            position,
            ..Self::default()
        }
    }

    /// Returns the record of a draw of item `position` into bin page `page`, whose shift takes the
    /// origin of `region` to `at`.
    pub(crate) fn binned(
        position: u32,
        page: u32,
        region: Rect<i32, Device>,
        at: Point<i32, Device>,
    ) -> Self {
        Self {
            position,
            page,
            shift: [
                (at.x - region.origin.x) as f32,
                (at.y - region.origin.y) as f32,
            ],
            region: [
                region.origin.x as f32,
                region.origin.y as f32,
                region.size.width as f32,
                region.size.height as f32,
            ],
            cells: [0; 4],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MarkDraw;
    use iceblink::geom::{Point, Rect, Size};

    #[test]
    fn a_bin_shift_takes_the_region_origin_to_its_place_in_the_page() {
        let draw = MarkDraw::binned(
            3,
            1,
            Rect::new(Point::new(40, 50), Size::new(10, 20)),
            Point::new(100, 0),
        );
        assert_eq!(draw.shift, [60.0, -50.0]);
        assert_eq!(draw.region, [40.0, 50.0, 10.0, 20.0]);
        assert_eq!(draw.page, 1);
    }
}
