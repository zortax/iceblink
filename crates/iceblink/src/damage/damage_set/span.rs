//! Edge arithmetic on damage rectangles.

// The merge reads rectangles as their four edges, and a capacity merge runs a few hundred of these
// operations for each rectangle it places.

use crate::geom::{Device, Point, Rect, Size};

/// A rectangle as its four edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Span {
    left: i32,
    top: i32,
    /// The right edge, outside the span.
    right: i32,
    /// The bottom edge, outside the span.
    bottom: i32,
}

impl Span {
    /// Returns the edges of `rect`.
    #[inline]
    pub(super) fn of(rect: Rect<i32, Device>) -> Self {
        Self {
            left: rect.left(),
            top: rect.top(),
            right: rect.right(),
            bottom: rect.bottom(),
        }
    }

    /// Returns the span with these edges.
    #[inline]
    pub(super) const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    #[inline]
    pub(super) const fn left(self) -> i32 {
        self.left
    }

    #[inline]
    pub(super) const fn top(self) -> i32 {
        self.top
    }

    #[inline]
    pub(super) const fn right(self) -> i32 {
        self.right
    }

    #[inline]
    pub(super) const fn bottom(self) -> i32 {
        self.bottom
    }

    /// Returns the rectangle with these edges.
    #[inline]
    pub(super) fn rect(self) -> Rect<i32, Device> {
        Rect::new(
            Point::new(self.left, self.top),
            Size::new(self.right - self.left, self.bottom - self.top),
        )
    }

    /// Reports whether the span holds no pixel, by the rule of [`Rect::is_empty`].
    #[inline]
    pub(super) fn is_empty(self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }

    /// Reports whether the two spans share a pixel, by the rule of [`Rect::intersects`].
    #[inline]
    pub(super) fn intersects(self, other: Self) -> bool {
        // Testing both axes with `&` compiles without a branch.
        (self.right.min(other.right) > self.left.max(other.left))
            & (self.bottom.min(other.bottom) > self.top.max(other.top))
    }

    /// Returns the smallest span that holds both, ignoring an empty one as [`Rect::union`] does.
    #[inline]
    pub(super) fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    /// Returns the smallest span that holds both, which must each hold a pixel.
    #[inline]
    pub(super) fn join(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    /// Returns the number of pixels the span covers, saturating.
    #[inline]
    pub(super) fn area(self) -> i64 {
        if self.is_empty() {
            return 0;
        }
        (i64::from(self.right) - i64::from(self.left))
            .saturating_mul(i64::from(self.bottom) - i64::from(self.top))
    }

    /// Returns the number of pixels the two spans share.
    #[inline]
    pub(super) fn overlap_area(self, other: Self) -> i64 {
        if !self.intersects(other) {
            return 0;
        }
        Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        }
        .area()
    }
}

/// Reports whether the two rectangles share a pixel, as [`Rect::intersects`] does.
#[inline]
pub(super) fn meets(left: Rect<i32, Device>, right: Rect<i32, Device>) -> bool {
    Span::of(left).intersects(Span::of(right))
}

/// Returns the smallest rectangle that holds both, as [`Rect::union`] does.
#[inline]
pub(super) fn join(left: Rect<i32, Device>, right: Rect<i32, Device>) -> Rect<i32, Device> {
    Span::of(left).union(Span::of(right)).rect()
}

/// Returns the number of pixels a rectangle covers, saturating.
pub(super) fn area(rect: Rect<i32, Device>) -> i64 {
    if rect.is_empty() {
        return 0;
    }
    i64::from(rect.width()).saturating_mul(i64::from(rect.height()))
}

/// Returns the parts of `rect` outside `held` as up to four disjoint rectangles.
///
/// The pieces are the rows above and below `held` across all of `rect`, and the columns beside
/// `held` between those rows.
pub(super) fn outside(
    rect: Rect<i32, Device>,
    held: Rect<i32, Device>,
) -> impl Iterator<Item = Rect<i32, Device>> {
    let (rect, held) = (Span::of(rect), Span::of(held));
    let span = |left: i32, top: i32, right: i32, bottom: i32| Span {
        left,
        top,
        right: right.max(left),
        bottom: bottom.max(top),
    };
    let top = rect.top.max(held.top);
    let bottom = rect.bottom.min(held.bottom);
    [
        span(rect.left, rect.top, rect.right, held.top.min(rect.bottom)),
        span(
            rect.left,
            held.bottom.max(rect.top),
            rect.right,
            rect.bottom,
        ),
        span(rect.left, top, held.left.min(rect.right), bottom),
        span(held.right.max(rect.left), top, rect.right, bottom),
    ]
    .into_iter()
    .filter(|piece| !piece.is_empty())
    .map(Span::rect)
}

/// Up to `N` disjoint pieces of a rectangle.
pub(super) struct Pieces<const N: usize> {
    rects: [Rect<i32, Device>; N],
    len: usize,
}

impl<const N: usize> Pieces<N> {
    pub(super) const EMPTY: Self = Self {
        rects: [Rect::ZERO; N],
        len: 0,
    };

    /// Returns the pieces.
    pub(super) fn rects(&self) -> &[Rect<i32, Device>] {
        &self.rects[..self.len]
    }

    /// Adds `rect`, or reports that `N` pieces leave no room for it.
    fn add(&mut self, rect: Rect<i32, Device>) -> bool {
        let Some(slot) = self.rects.get_mut(self.len) else {
            return false;
        };
        *slot = rect;
        self.len += 1;
        true
    }
}

/// Stores the part of `rect` outside every rectangle of `held` as disjoint pieces in `pieces`,
/// and reports whether it took `N` pieces or fewer.
pub(super) fn remainder<const N: usize>(
    rect: Rect<i32, Device>,
    held: &[Rect<i32, Device>],
    pieces: &mut Pieces<N>,
) -> bool {
    let mut next = Pieces::<N>::EMPTY;
    pieces.len = 0;
    if !pieces.add(rect) {
        return false;
    }
    for one in held {
        next.len = 0;
        for &piece in pieces.rects() {
            if meets(piece, *one) {
                for part in outside(piece, *one) {
                    if !next.add(part) {
                        return false;
                    }
                }
            } else if !next.add(piece) {
                return false;
            }
        }
        core::mem::swap(pieces, &mut next);
    }
    true
}

#[cfg(test)]
mod tests {
    use crate::geom::{Device, Point, Rect, Size};

    use super::{area, outside};

    fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect<i32, Device> {
        Rect::new(Point::new(x, y), Size::new(width, height))
    }

    #[test]
    fn what_lies_outside_a_held_rectangle_covers_the_rest_and_nothing_twice() {
        let whole = rect(0, 0, 100, 100);
        let held = rect(30, 40, 50, 20);
        let pieces: Vec<_> = outside(whole, held).collect();
        assert_eq!(pieces.len(), 4);
        let covered: i64 = pieces.iter().map(|piece| area(*piece)).sum();
        assert_eq!(covered, area(whole) - area(held));
        for (index, one) in pieces.iter().enumerate() {
            assert!(!one.intersects(held));
            for other in &pieces[index + 1..] {
                assert!(!one.intersects(*other));
            }
        }
    }
}
