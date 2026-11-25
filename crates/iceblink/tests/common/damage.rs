//! Builders and checks shared by the damage tests.

use std::collections::HashSet;

use iceblink::damage::DamageSet;
use iceblink::geom::{Device, Point, Rect, Size};

/// Returns a rectangle in whole device pixels.
pub(crate) fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect<i32, Device> {
    Rect::new(Point::new(x, y), Size::new(width, height))
}

/// Returns every pixel a rectangle covers.
pub(crate) fn pixels(rect: Rect<i32, Device>) -> impl Iterator<Item = (i32, i32)> {
    (rect.top()..rect.bottom()).flat_map(move |y| (rect.left()..rect.right()).map(move |x| (x, y)))
}

/// Returns every pixel a set covers.
pub(crate) fn covered<const N: usize>(set: &DamageSet<N>) -> HashSet<(i32, i32)> {
    set.rects().iter().copied().flat_map(pixels).collect()
}

/// Asserts that a set is within its capacity and holds no two overlapping rectangles.
pub(crate) fn assert_disjoint<const N: usize>(set: &DamageSet<N>) {
    let rects = set.rects();
    assert!(rects.len() <= N, "the set exceeded its own capacity");
    for (index, left) in rects.iter().enumerate() {
        for right in &rects[index + 1..] {
            assert!(!left.intersects(*right), "{left:?} and {right:?} overlap");
        }
    }
}

/// A small deterministic generator, so long random runs are reproducible.
pub(crate) struct Xorshift(pub(crate) u64);

impl Xorshift {
    /// Returns the next value in the sequence.
    pub(crate) fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Returns a value below `limit`.
    pub(crate) fn below(&mut self, limit: i32) -> i32 {
        (self.next() % limit as u64) as i32
    }
}
