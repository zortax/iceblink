//! Merging nearby damage rectangles into their bounds.

use crate::geom::{Device, Rect};

use crate::damage::damage_set::span;

/// The most pixels [`DamageSet::coalesce_dense`](crate::damage::DamageSet::coalesce_dense) adds
/// when it merges a group of rectangles, as a share of the pixels the group covers.
// Each rectangle is one pass over the frame's batches. Measured on a discrete GPU at 1280x800 with
// a thousand quads and three thousand discs per damaged panel, each rectangle less saved about
// 15 µs of submission and 20 to 25 µs of submission and GPU time. Two panels of 620x380 whose
// bounds were half again their area drew faster as one rectangle (152 µs against 161 µs).
pub const COALESCE_WASTE_SHARE: f64 = 0.5;

/// A group of held rectangles that merges into its bounds.
pub(super) struct Group<const N: usize> {
    /// Which held rectangles the group takes.
    pub(super) taken: [bool; N],
    /// The rectangle that replaces them.
    pub(super) bounds: Rect<i32, Device>,
    /// The pixels the taken rectangles covered before any earlier merge.
    pub(super) covered: i64,
}

/// Returns the group whose merge adds the fewest pixels, if that is within
/// [`COALESCE_WASTE_SHARE`].
///
/// A group starts from a pair of `rects` and takes in every rectangle its bounds overlap. `covered`
/// holds the pixels each rectangle covered before any earlier merge. Ties go to the first pair in
/// order.
pub(super) fn cheapest_group<const N: usize>(
    rects: &[Rect<i32, Device>],
    covered: &[i64; N],
) -> Option<Group<N>> {
    let mut best: Option<(i64, Group<N>)> = None;
    for left in 0..rects.len() {
        for right in left + 1..rects.len() {
            let mut taken = [false; N];
            taken[left] = true;
            taken[right] = true;
            let mut bounds = span::join(rects[left], rects[right]);
            let mut grew = true;
            while grew {
                grew = false;
                for (index, rect) in rects.iter().enumerate() {
                    if !taken[index] && span::meets(*rect, bounds) {
                        taken[index] = true;
                        bounds = span::join(bounds, *rect);
                        grew = true;
                    }
                }
            }
            let group_covered = taken
                .iter()
                .zip(covered)
                .filter(|(taken, _)| **taken)
                .map(|(_, covered)| *covered)
                .fold(0i64, i64::saturating_add);
            let extra = span::area(bounds).saturating_sub(group_covered);
            if extra as f64 > group_covered as f64 * COALESCE_WASTE_SHARE {
                continue;
            }
            if best.as_ref().is_none_or(|(least, _)| extra < *least) {
                let group = Group {
                    taken,
                    bounds,
                    covered: group_covered,
                };
                best = Some((extra, group));
            }
        }
    }
    best.map(|(_, group)| group)
}
