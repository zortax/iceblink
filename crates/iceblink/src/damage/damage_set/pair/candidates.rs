//! The rectangles a capacity merge chooses between, and the waste of merging two of them.

use crate::damage::damage_set::span::Span;

/// The rectangles a capacity merge chooses between: a full set plus the one it has no room for,
/// at index `N`.
// One array per edge lets a test against every held rectangle run as one loop.
pub(super) struct Candidates<const N: usize> {
    pub(super) lefts: [i32; N],
    pub(super) tops: [i32; N],
    pub(super) rights: [i32; N],
    pub(super) bottoms: [i32; N],
    /// The held rectangles' areas.
    pub(super) areas: [i64; N],
    /// The incoming rectangle's edges.
    pub(super) incoming: Span,
    /// The incoming rectangle's area.
    pub(super) incoming_area: i64,
}

impl<const N: usize> Candidates<N> {
    /// The number of rectangles to choose between.
    pub(super) const COUNT: usize = N + 1;

    /// Returns the edges of the rectangle at `index`.
    #[inline]
    pub(super) fn at(&self, index: usize) -> Span {
        if index == N {
            self.incoming
        } else {
            Span::new(
                self.lefts[index],
                self.tops[index],
                self.rights[index],
                self.bottoms[index],
            )
        }
    }

    /// Returns the area of the rectangle at `index`.
    #[inline]
    pub(super) fn area(&self, index: usize) -> i64 {
        if index == N {
            self.incoming_area
        } else {
            self.areas[index]
        }
    }

    /// Returns a mask with the bit of each rectangle that shares a pixel with `union` set.
    #[inline]
    fn hits(&self, union: Span) -> u64 {
        let mut mask = u64::from(self.incoming.intersects(union)) << N;
        for index in 0..N {
            let across =
                self.rights[index].min(union.right()) > self.lefts[index].max(union.left());
            let down = self.bottoms[index].min(union.bottom()) > self.tops[index].max(union.top());
            mask |= u64::from(across & down) << index;
        }
        mask
    }

    /// Returns the pixels that merging the rectangles at `left` and `right` adds, counting every
    /// other rectangle the union grows over, or `None` if that is sure to be `limit` or more.
    pub(super) fn closure_waste(&self, left: usize, right: usize, limit: i64) -> Option<i64> {
        let pair = (1_u64 << left) | (1_u64 << right);
        // Every candidate holds a pixel, so the unions below need no test for empty spans.
        let mut union = self.at(left).join(self.at(right));
        // The closure's waste is the part of its bounding box no candidate covers. The candidates
        // are disjoint, so that is at least the part of the pair's union they leave uncovered.
        let (hits, covered) = self.overlaps(union);
        if union.area().saturating_sub(covered) >= limit {
            return None;
        }
        let mut fresh = hits & !pair;
        // The common case: a union that meets nothing else.
        if fresh == 0 {
            return Some(
                union
                    .area()
                    .saturating_sub(self.area(left))
                    .saturating_sub(self.area(right)),
            );
        }
        // The union of the taken rectangles is their bounding box, so the closure is the same in
        // every visiting order.
        let mut taken = pair;
        while fresh != 0 {
            taken |= fresh;
            let mut rest = fresh;
            while rest != 0 {
                union = union.join(self.at(rest.trailing_zeros() as usize));
                rest &= rest - 1;
            }
            fresh = self.hits(union) & !taken;
        }
        let mut covered = 0_i64;
        let mut rest = taken;
        while rest != 0 {
            covered = covered.saturating_add(self.area(rest.trailing_zeros() as usize));
            rest &= rest - 1;
        }
        Some(union.area().saturating_sub(covered))
    }

    /// Returns a mask with the bit of each rectangle that shares a pixel with `union` set, and
    /// the pixels of `union` they cover.
    #[inline]
    fn overlaps(&self, union: Span) -> (u64, i64) {
        let incoming = self.incoming.overlap_area(union);
        let mut mask = u64::from(incoming > 0) << N;
        let mut covered = incoming;
        for index in 0..N {
            let width = self.rights[index].min(union.right()) - self.lefts[index].max(union.left());
            let height =
                self.bottoms[index].min(union.bottom()) - self.tops[index].max(union.top());
            let overlap = if width > 0 && height > 0 {
                i64::from(width) * i64::from(height)
            } else {
                0
            };
            mask |= u64::from(overlap > 0) << index;
            covered = covered.saturating_add(overlap);
        }
        (mask, covered)
    }
}
