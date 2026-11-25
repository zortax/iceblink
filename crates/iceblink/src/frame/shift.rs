//! Pixels of the kept target moved by whole pixels.

use crate::damage::DamageSet;
use crate::geom::{Point, Rect, Size};
use crate::geometry::DeviceRect;

/// A move of the pixels of a region of the kept target, by whole device pixels.
///
/// The renderer copies the source to the destination before it draws the frame, as if through a
/// temporary. The rest of the region is exposed, and the frame's draw damage holds it. Content
/// scrolled down by three pixels moves up, so its `by` is `[0, -3]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shift {
    /// The pixels that move, already cut to the output.
    pub region: DeviceRect,
    /// How far they move, in whole device pixels.
    pub by: [i32; 2],
}

impl Shift {
    /// Returns where the moved pixels land, `(region + by) ∩ region`, or `None` when nothing of
    /// the region stays in it.
    pub fn destination(&self) -> Option<DeviceRect> {
        self.moved(self.region).intersection(self.region)
    }

    /// Returns where the moved pixels come from: the destination moved back by `by`.
    pub fn source(&self) -> Option<DeviceRect> {
        self.destination().map(|to| {
            Rect::new(
                Point::new(to.origin.x - self.by[0], to.origin.y - self.by[1]),
                to.size,
            )
        })
    }

    /// Calls `band` with each part of the region the move leaves exposed.
    ///
    /// There are at most two bands, a horizontal one across the region's whole width and a
    /// vertical one beside the destination, and they do not overlap. A move that leaves nothing
    /// of the region in it exposes the whole region as one band.
    pub fn for_each_exposed(&self, mut band: impl FnMut(DeviceRect)) {
        let mut out = |rect: DeviceRect| {
            if !rect.is_empty() {
                band(rect);
            }
        };
        let Some(kept) = self.destination() else {
            out(self.region);
            return;
        };

        let region = self.region;
        let (top, bottom) = (region.origin.y, region.origin.y + region.size.height);
        if kept.origin.y > top {
            out(Rect::new(
                region.origin,
                Size::new(region.size.width, kept.origin.y - top),
            ));
        }
        let kept_bottom = kept.origin.y + kept.size.height;
        if kept_bottom < bottom {
            out(Rect::new(
                Point::new(region.origin.x, kept_bottom),
                Size::new(region.size.width, bottom - kept_bottom),
            ));
        }

        // Only over the rows the horizontal band left, so the bands stay disjoint.
        let (left, right) = (region.origin.x, region.origin.x + region.size.width);
        if kept.origin.x > left {
            out(Rect::new(
                Point::new(left, kept.origin.y),
                Size::new(kept.origin.x - left, kept.size.height),
            ));
        }
        let kept_right = kept.origin.x + kept.size.width;
        if kept_right < right {
            out(Rect::new(
                Point::new(kept_right, kept.origin.y),
                Size::new(right - kept_right, kept.size.height),
            ));
        }
    }

    /// Adds every exposed band to `damage`.
    pub fn expose_into<const N: usize>(&self, damage: &mut DamageSet<N>) {
        self.for_each_exposed(|band| damage.absorb(band));
    }

    /// Returns where the pixels of `rect` are after the move.
    ///
    /// Only the part of `rect` inside the region moves, and only the part that lands inside the
    /// region stays. The result is `None` when nothing stays.
    pub fn carry(&self, rect: DeviceRect) -> Option<DeviceRect> {
        let inside = rect.intersection(self.region)?;
        self.moved(inside).intersection(self.region)
    }

    /// Adds `owed`, measured before the move, to `damage` at its place after the move.
    ///
    /// The part of `owed` outside the region stays where it is. The part inside moves with the
    /// pixels, and what the move takes out of the region is dropped. A full `owed` makes `damage`
    /// full.
    pub fn carry_set<const N: usize, const M: usize>(
        &self,
        owed: &DamageSet<N>,
        damage: &mut DamageSet<M>,
    ) {
        if owed.is_full() {
            damage.set_full();
            return;
        }
        for &rect in owed.rects() {
            self.for_each_outside(rect, |piece| damage.absorb(piece));
            if let Some(carried) = self.carry(rect) {
                damage.absorb(carried);
            }
        }
    }

    /// Returns `rect` moved by `by`.
    fn moved(&self, rect: DeviceRect) -> DeviceRect {
        Rect::new(
            Point::new(rect.origin.x + self.by[0], rect.origin.y + self.by[1]),
            rect.size,
        )
    }

    /// Calls `piece` with the parts of `rect` outside the region, at most four and disjoint.
    fn for_each_outside(&self, rect: DeviceRect, mut piece: impl FnMut(DeviceRect)) {
        let Some(inside) = rect.intersection(self.region) else {
            if !rect.is_empty() {
                piece(rect);
            }
            return;
        };
        let mut out = |left: i32, top: i32, right: i32, bottom: i32| {
            if right > left && bottom > top {
                piece(Rect::new(
                    Point::new(left, top),
                    Size::new(right - left, bottom - top),
                ));
            }
        };

        let (left, top) = (rect.origin.x, rect.origin.y);
        let (right, bottom) = (left + rect.size.width, top + rect.size.height);
        let (in_top, in_bottom) = (inside.origin.y, inside.origin.y + inside.size.height);
        let (in_left, in_right) = (inside.origin.x, inside.origin.x + inside.size.width);
        out(left, top, right, in_top);
        out(left, in_bottom, right, bottom);
        out(left, in_top, in_left, in_bottom);
        out(in_right, in_top, right, in_bottom);
    }
}
