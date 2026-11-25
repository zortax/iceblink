//! Adding rectangles to a damage set.

use crate::geom::{Device, Rect};

use crate::damage::damage_set::span::Pieces;
use crate::damage::damage_set::{DamageSet, pair, span};

impl<const N: usize> DamageSet<N> {
    /// Adds `rect` to the set, keeping the rectangles disjoint.
    ///
    /// Nothing changes if the set is full, if `rect` is empty or outside the bound, or if a held
    /// rectangle contains `rect`. Otherwise every held rectangle that overlaps `rect` closely is
    /// merged into it first. Two rectangles overlap closely when their union adds no more pixels
    /// than the larger of the two covers.
    ///
    /// The merged rectangle is then added:
    ///
    /// - If it meets no held rectangle, it is added as it is.
    /// - If its part outside the held rectangles takes no more disjoint pieces than the capacity,
    ///   the pieces are added.
    /// - Otherwise it is [absorbed whole](DamageSet::absorb_whole).
    ///
    /// Adding a rectangle to a set at capacity first merges the pair of rectangles whose union
    /// wastes the fewest pixels. Every held rectangle that union overlaps is merged in too, and
    /// its pixels count toward the waste.
    pub fn absorb(&mut self, rect: Rect<i32, Device>) {
        if self.full || rect.is_empty() {
            return;
        }
        let Some(rect) = self.bounded(rect) else {
            return;
        };
        if self.contains(rect) {
            return;
        }
        // Merge the held rectangles that overlap it closely enough to be one region with it.
        let mut rect = rect;
        while let Some(index) = self
            .rects()
            .iter()
            .position(|held| span::meets(*held, rect) && pair::worth_merging(*held, rect))
        {
            let held = self.rects[index];
            self.remove(index);
            rect = span::join(held, rect);
        }
        // The common case: it meets nothing held.
        if !self.rects().iter().any(|held| span::meets(*held, rect)) {
            self.push(rect);
            return;
        }
        // Two strips meeting at a corner, such as the bands a widened window exposes, would merge
        // into everything between them, so the remainder is added in pieces instead.
        let mut pieces = Pieces::<N>::EMPTY;
        if !span::remainder(rect, self.rects(), &mut pieces) {
            self.absorb_whole(rect);
            return;
        }
        // Past the capacity, each piece makes room by merging the least wasteful pair.
        for &piece in pieces.rects() {
            // A merge made room for an earlier piece may have grown over this one.
            if self.rects().iter().any(|held| span::meets(*held, piece)) {
                self.absorb_whole(piece);
            } else {
                self.push(piece);
            }
        }
    }

    /// Adds `rect` so that one held rectangle contains all of it.
    ///
    /// Every held rectangle that `rect` overlaps is merged into it, and so is every held rectangle
    /// the growing union then overlaps. Nothing changes if the set is full, `rect` is empty or
    /// outside the bound, or a held rectangle already contains `rect`.
    pub fn absorb_whole(&mut self, rect: Rect<i32, Device>) {
        if self.full || rect.is_empty() {
            return;
        }
        let Some(rect) = self.bounded(rect) else {
            return;
        };
        if self.contains(rect) {
            return;
        }
        let mut merged = rect;
        let mut index = 0;
        while index < self.len {
            if span::meets(self.rects[index], merged) {
                merged = span::join(merged, self.rects[index]);
                self.remove(index);
                // The union grew, so a rectangle already passed over may now meet it.
                index = 0;
            } else {
                index += 1;
            }
        }
        self.push(merged);
    }

    /// Appends `rect`, first merging the least wasteful pair if the set is at capacity.
    ///
    /// `rect` must not overlap a held rectangle.
    pub(super) fn push(&mut self, rect: Rect<i32, Device>) {
        if self.len < N {
            self.rects[self.len] = rect;
            self.len += 1;
            return;
        }

        let (left, right) = pair::least_wasted_pair(&self.rects, rect);
        if right == N {
            let union = span::join(self.rects[left], rect);
            self.remove(left);
            self.absorb_whole(union);
        } else {
            let union = span::join(self.rects[left], self.rects[right]);
            self.remove(right);
            self.remove(left);
            self.push(rect);
            self.absorb_whole(union);
        }
    }

    /// Removes the rectangle at `index`, keeping the order of the rest.
    pub(super) fn remove(&mut self, index: usize) {
        self.rects.copy_within(index + 1..self.len, index);
        self.len -= 1;
    }
}
