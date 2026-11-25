//! The choice of which damage rectangles to merge.

mod candidates;

use crate::geom::{Device, Rect};

use crate::damage::damage_set::pair::candidates::Candidates;
use crate::damage::damage_set::span::Span;

/// Reports whether two overlapping rectangles merge into their union.
///
/// They merge when the union adds no more pixels than the larger of the two covers. Two strips
/// that only meet at a corner are kept apart.
pub(super) fn worth_merging(held: Rect<i32, Device>, incoming: Rect<i32, Device>) -> bool {
    let (held, incoming) = (Span::of(held), Span::of(incoming));
    let covered = held
        .area()
        .saturating_add(incoming.area())
        .saturating_sub(held.overlap_area(incoming));
    let waste = held.union(incoming).area().saturating_sub(covered);
    waste <= held.area().max(incoming.area())
}

/// Returns the two rectangles of `existing` plus `incoming` whose union wastes the fewest pixels.
///
/// A union that overlaps further rectangles takes them in as well, and the waste is counted over
/// all of them. Ties go to the first pair in order, and the search stops at a pair that wastes
/// nothing.
///
/// The indices are into `existing`, with `N` standing for `incoming`, and the first is the smaller.
/// `existing` and `incoming` must be pairwise disjoint, and `N` must be at least 1 and below 64.
pub(super) fn least_wasted_pair<const N: usize>(
    existing: &[Rect<i32, Device>; N],
    incoming: Rect<i32, Device>,
) -> (usize, usize) {
    const {
        assert!(
            N >= 1 && N < 64,
            "a merge needs two rectangles to choose between, and the closure marks what it takes \
             in one 64-bit mask"
        );
    };
    let spans = existing.map(Span::of);
    let incoming = Span::of(incoming);
    let candidates = Candidates {
        lefts: spans.map(Span::left),
        tops: spans.map(Span::top),
        rights: spans.map(Span::right),
        bottoms: spans.map(Span::bottom),
        areas: spans.map(Span::area),
        incoming,
        incoming_area: incoming.area(),
    };

    // A closure's waste is at least the pair's union less every candidate, which is quicker to
    // find than the closure's own bound.
    let total = (0..Candidates::<N>::COUNT).fold(0_i64, |sum, index| {
        sum.saturating_add(candidates.area(index))
    });
    let mut best = (0, 1);
    let mut best_waste = i64::MAX;
    for left in 0..Candidates::<N>::COUNT {
        for right in (left + 1)..Candidates::<N>::COUNT {
            // Ties go to the earlier pair, so a later one must waste less than the best.
            let union = candidates.at(left).join(candidates.at(right));
            if union.area().saturating_sub(total) >= best_waste {
                continue;
            }
            let Some(waste) = candidates.closure_waste(left, right, best_waste) else {
                continue;
            };
            if waste < best_waste {
                best_waste = waste;
                best = (left, right);
                if waste <= 0 {
                    debug_assert!(
                        waste == 0,
                        "the candidates overlap, so the early stop is wrong"
                    );
                    return best;
                }
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use crate::geom::{Device, Point, Rect, Size};

    use super::{least_wasted_pair, worth_merging};

    fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect<i32, Device> {
        Rect::new(Point::new(x, y), Size::new(width, height))
    }

    #[test]
    fn a_merge_that_would_swallow_a_third_rectangle_is_counted_with_it() {
        // Two slivers either side of a column waste a pixel as a pair, and take the column with
        // them as a closure.
        let existing = [
            rect(1985, 74, 15, 1049),
            rect(319, 1123, 2241, 18),
            rect(319, 1122, 1666, 1),
            rect(2000, 1122, 560, 1),
        ];
        let (left, right) = least_wasted_pair(&existing, rect(2545, 116, 15, 1023));
        assert_ne!(
            (left, right),
            (2, 3),
            "the slivers' union crosses the column"
        );
    }

    #[test]
    fn the_search_stops_at_the_first_pair_that_wastes_nothing() {
        let existing = [
            rect(0, 0, 10, 10),
            rect(10, 0, 10, 10),
            rect(100, 0, 10, 10),
            rect(110, 0, 10, 10),
        ];
        assert_eq!(least_wasted_pair(&existing, rect(500, 500, 4, 4)), (0, 1));
    }

    #[test]
    fn strips_meeting_at_a_corner_are_kept_apart_and_overlapping_pieces_are_merged() {
        assert!(!worth_merging(rect(90, 0, 10, 100), rect(0, 90, 100, 10)));
        assert!(worth_merging(rect(0, 0, 10, 10), rect(5, 5, 10, 10)));
        assert!(worth_merging(rect(90, 0, 10, 100), rect(88, 0, 4, 100)));
    }
}
