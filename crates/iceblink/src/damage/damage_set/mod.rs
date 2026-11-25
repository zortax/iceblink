//! A bounded set of pairwise disjoint damage rectangles.

mod absorb;
mod coalesce;
mod override_env;
mod pair;
mod span;

use crate::geom::{Device, Rect};

pub use crate::damage::damage_set::coalesce::COALESCE_WASTE_SHARE;
pub use crate::damage::damage_set::override_env::full_damage_forced;

/// The default capacity of a [`DamageSet`].
// Every rectangle is its own render pass. At four, the strips along an edge dragged between two
// scroll bars and a dock merged into the whole surface on every step, a third more primitives per
// step. Eight saved nothing over six.
pub const MAX_DAMAGE: usize = 6;

/// A bounded set of pairwise disjoint rectangles covering the device pixels that must be redrawn.
///
/// The set holds at most `N` rectangles, [`MAX_DAMAGE`] by default. No pixel is covered by two
/// rectangles. When a rectangle does not fit, the set merges rectangles until it does, down to a
/// single bounding rectangle.
///
/// A set can be full, meaning the whole surface must be redrawn. A full set holds no rectangles.
///
/// A set can have a bound, and every rectangle it absorbs is cut to that bound.
///
/// ```
/// use iceblink::damage::DamageSet;
/// use iceblink::geom::{Device, Point, Rect, Size};
///
/// let mut damage = DamageSet::<4>::new();
/// damage.absorb(Rect::new(Point::new(0, 0), Size::new(10, 10)));
/// damage.absorb(Rect::new(Point::new(5, 5), Size::new(10, 10)));
/// // The two overlapped, so one rectangle now contains both.
/// assert_eq!(damage.len(), 1);
/// assert!(damage.rects()[0].contains_rect(Rect::new(Point::new(0, 0), Size::new(15, 15))));
///
/// let far: Rect<i32, Device> = Rect::new(Point::new(500, 500), Size::new(4, 4));
/// damage.absorb(far);
/// assert_eq!(damage.len(), 2);
/// assert!(damage.intersects(far));
/// ```
#[derive(Clone, Copy)]
pub struct DamageSet<const N: usize = MAX_DAMAGE> {
    /// The held rectangles in the first `len` entries.
    rects: [Rect<i32, Device>; N],
    len: usize,
    full: bool,
    bound: Option<Rect<i32, Device>>,
}

impl<const N: usize> DamageSet<N> {
    /// The most rectangles the set holds.
    pub const CAPACITY: usize = N;

    // A set of capacity zero has nowhere to merge to.
    const CAPACITY_IS_USABLE: () = assert!(N >= 1);

    /// Creates an empty set.
    pub const fn new() -> Self {
        () = Self::CAPACITY_IS_USABLE;
        Self {
            rects: [Rect::ZERO; N],
            len: 0,
            full: false,
            bound: None,
        }
    }

    /// Sets the region every rectangle absorbed from now on is cut to, or stops cutting with
    /// `None`.
    ///
    /// The rectangles already held are kept as they are.
    pub const fn set_bound(&mut self, bound: Option<Rect<i32, Device>>) {
        self.bound = bound;
    }

    /// Returns the region absorbed rectangles are cut to, if any.
    pub const fn bound(&self) -> Option<Rect<i32, Device>> {
        self.bound
    }

    /// Cuts `rect` to the bound, or returns `None` if none of it is inside.
    fn bounded(&self, rect: Rect<i32, Device>) -> Option<Rect<i32, Device>> {
        match self.bound {
            Some(bound) => rect.intersection(bound),
            None => Some(rect),
        }
    }

    /// Creates a full set, which covers the whole surface.
    pub const fn full() -> Self {
        let mut set = Self::new();
        set.full = true;
        set
    }

    /// Creates an empty set, or a full one if [`full_damage_forced`] reports that the environment
    /// forces full damage.
    pub fn for_frame() -> Self {
        if full_damage_forced() {
            Self::full()
        } else {
            Self::new()
        }
    }

    /// Reports whether the whole surface must be redrawn.
    pub const fn is_full(&self) -> bool {
        self.full
    }

    /// Marks the whole surface as damaged and drops the held rectangles.
    pub fn set_full(&mut self) {
        self.full = true;
        self.len = 0;
    }

    /// Reports whether nothing needs redrawing.
    pub const fn is_empty(&self) -> bool {
        !self.full && self.len == 0
    }

    /// Returns how many rectangles the set holds, which is zero for a full set.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns the held rectangles, pairwise disjoint and in no particular order.
    pub fn rects(&self) -> &[Rect<i32, Device>] {
        &self.rects[..self.len]
    }

    /// Cuts every held rectangle to `surface` and drops the ones wholly outside it.
    ///
    /// A full set is left as it is.
    pub fn clip_to(&mut self, surface: Rect<i32, Device>) {
        if self.full {
            return;
        }
        let mut kept = 0;
        for index in 0..self.len {
            if let Some(cut) = self.rects[index].intersection(surface)
                && !cut.is_empty()
            {
                self.rects[kept] = cut;
                kept += 1;
            }
        }
        self.len = kept;
    }

    /// Empties the set and clears the full flag.
    pub fn clear(&mut self) {
        self.len = 0;
        self.full = false;
    }

    /// Merges groups of nearby rectangles into their bounds, and returns whether it merged any.
    ///
    /// A group starts from a pair of rectangles and takes in every rectangle its bounds overlap.
    /// It merges when its bounds add at most [`COALESCE_WASTE_SHARE`] of the pixels it covers,
    /// where pixels added by an earlier merge do not count as covered. The group that adds the
    /// fewest pixels merges first, with ties going to the first pair in order, and merging repeats
    /// until no group is within the share.
    ///
    /// The set stays disjoint and covers every pixel it covered. A full set and a set of fewer
    /// than two rectangles are left as they are.
    pub fn coalesce_dense(&mut self) -> bool {
        if self.full || self.len < 2 {
            return false;
        }
        let mut covered = [0i64; N];
        for (index, rect) in self.rects().iter().enumerate() {
            covered[index] = span::area(*rect);
        }
        let mut merged = false;
        while let Some(group) = coalesce::cheapest_group(self.rects(), &covered) {
            let mut kept = 0;
            for index in 0..self.len {
                if !group.taken[index] {
                    self.rects[kept] = self.rects[index];
                    covered[kept] = covered[index];
                    kept += 1;
                }
            }
            self.rects[kept] = group.bounds;
            covered[kept] = group.covered;
            self.len = kept + 1;
            merged = true;
        }
        merged
    }

    /// Absorbs every rectangle of `other`, or becomes full if `other` is full.
    pub fn absorb_set(&mut self, other: &Self) {
        if other.full {
            self.set_full();
            return;
        }
        for rect in other.rects() {
            self.absorb(*rect);
        }
    }

    /// Reports whether one held rectangle contains all of `rect`.
    ///
    /// A full set contains every rectangle. A rectangle that spans two held rectangles is
    /// contained by neither.
    pub fn contains(&self, rect: Rect<i32, Device>) -> bool {
        if self.full {
            return true;
        }
        self.rects().iter().any(|held| held.contains_rect(rect))
    }

    /// Reports whether `rect` shares a pixel with the damage.
    ///
    /// A full set intersects every non-empty rectangle, and an empty rectangle intersects nothing.
    pub fn intersects(&self, rect: Rect<i32, Device>) -> bool {
        if self.full {
            return !rect.is_empty();
        }
        self.rects().iter().any(|held| span::meets(*held, rect))
    }

    /// Returns the smallest rectangle containing every held rectangle, or `None` if the set is
    /// empty or full.
    pub fn bounds(&self) -> Option<Rect<i32, Device>> {
        if self.full || self.len == 0 {
            return None;
        }
        Some(
            self.rects()
                .iter()
                .fold(Rect::ZERO, |accumulated, rect| accumulated.union(*rect)),
        )
    }

    /// Returns the exact number of pixels the set covers, saturating, or `None` if the set is full.
    pub fn area(&self) -> Option<i64> {
        if self.full {
            return None;
        }
        Some(
            self.rects()
                .iter()
                .map(|rect| span::area(*rect))
                .fold(0i64, i64::saturating_add),
        )
    }
}

impl<const N: usize> Default for DamageSet<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Compares the full flag and the held rectangles in order, so sets holding the same rectangles
/// in another order are unequal.
impl<const N: usize> PartialEq for DamageSet<N> {
    fn eq(&self, other: &Self) -> bool {
        self.full == other.full && self.rects() == other.rects()
    }
}

impl<const N: usize> Eq for DamageSet<N> {}

impl<const N: usize> core::fmt::Debug for DamageSet<N> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DamageSet")
            .field("full", &self.full)
            .field("rects", &self.rects())
            .finish()
    }
}
