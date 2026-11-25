//! Damage rectangles: the ones a frame redraws, and the rows they cover.

use std::ops::Range;

use iceblink::damage::DamageSet;
use iceblink::geom::{Device, Rect};

/// Returns the rectangles of `damage` within `used`, or `used` whole when the damage is full.
pub(crate) fn rects(damage: &DamageSet, used: Rect<i32, Device>) -> Vec<Rect<i32, Device>> {
    if used.is_empty() {
        return Vec::new();
    }
    if damage.is_full() {
        return vec![used];
    }
    damage
        .rects()
        .iter()
        .filter_map(|rect| rect.intersection(used))
        .filter(|rect| !rect.is_empty())
        .collect()
}

/// Returns the rectangles of `damage` within `used`, grown to cover the `backdrops` sources the
/// damage reaches.
///
/// Each source the damage reaches is added whole, and the rectangles inside it are dropped.
pub(crate) fn rects_covering_backdrops(
    damage: &DamageSet,
    used: Rect<i32, Device>,
    backdrops: &[Rect<i32, Device>],
) -> Vec<Rect<i32, Device>> {
    let mut rects = rects(damage, used);
    if backdrops.is_empty() || rects.is_empty() {
        return rects;
    }

    // A backdrop the damage does not reach reads nothing this frame, so it is left alone.
    for source in backdrops {
        let Some(source) = source.intersection(used) else {
            continue;
        };
        if source.is_empty() || !rects.iter().any(|rect| rect.intersects(source)) {
            continue;
        }
        rects.retain(|rect| !source.contains_rect(*rect));
        if !rects.iter().any(|rect| rect.contains_rect(source)) {
            rects.push(source);
        }
    }
    rects
}

/// Returns how many pixels `rect` covers.
pub(crate) fn area(rect: Rect<i32, Device>) -> u64 {
    rect.size.width.max(0) as u64 * rect.size.height.max(0) as u64
}

/// Returns one band of every row of a target `height` rows tall.
pub(crate) fn every_row(height: u32) -> Vec<Range<u32>> {
    core::iter::once(0..height).collect()
}

/// Replaces `into` with the rows `rects` cover, as sorted bands that neither overlap nor touch.
pub(crate) fn rows_of(rects: &[Rect<i32, Device>], into: &mut Vec<Range<u32>>) {
    into.clear();
    into.extend(rects.iter().filter_map(|rect| {
        let top = rect.origin.y.max(0) as u32;
        let bottom = rect.origin.y.saturating_add(rect.size.height).max(0) as u32;
        (top < bottom).then_some(top..bottom)
    }));
    into.sort_unstable_by_key(|band| band.start);
    // Merged in place, with the surviving bands at the front. Bands that meet are merged too, so
    // they make one copy.
    let mut kept = 0;
    for index in 0..into.len() {
        if kept > 0 && into[index].start <= into[kept - 1].end {
            into[kept - 1].end = into[kept - 1].end.max(into[index].end);
        } else {
            into[kept] = into[index].clone();
            kept += 1;
        }
    }
    into.truncate(kept);
}

#[cfg(test)]
mod tests {
    use iceblink::damage::DamageSet;
    use iceblink::geom::{Device, Point, Rect, Size};

    use super::{area, rects, rects_covering_backdrops, rows_of};

    fn used() -> Rect<i32, Device> {
        Rect::new(Point::new(0, 0), Size::new(128, 128))
    }

    #[test]
    fn a_rectangle_reaching_past_the_surface_is_cut_to_it() {
        let mut damage: DamageSet = DamageSet::new();
        damage.absorb(Rect::new(Point::new(100, 100), Size::new(200, 200)));
        assert_eq!(
            rects(&damage, used()),
            vec![Rect::new(Point::new(100, 100), Size::new(28, 28))]
        );
    }

    #[test]
    fn a_rectangle_touching_a_backdrop_grows_to_cover_what_it_reads() {
        // One row lights up under a panel that reads the width of the window.
        let mut damage: DamageSet = DamageSet::new();
        damage.absorb(Rect::new(Point::new(10, 100), Size::new(60, 4)));
        let panel = Rect::new(Point::new(0, 96), Size::new(128, 20));

        let planned = rects_covering_backdrops(&damage, used(), &[panel]);

        assert_eq!(planned, vec![panel], "the panel's whole read is redrawn");
    }

    #[test]
    fn the_rectangles_stay_disjoint_so_no_pixel_is_redrawn_twice() {
        let mut damage: DamageSet = DamageSet::new();
        damage.absorb(Rect::new(Point::new(0, 0), Size::new(20, 20)));
        damage.absorb(Rect::new(Point::new(10, 10), Size::new(20, 20)));
        damage.absorb(Rect::new(Point::new(60, 60), Size::new(10, 10)));
        let planned = rects(&damage, used());
        assert_eq!(planned.len(), 2);
        assert_eq!(
            planned.iter().map(|rect| area(*rect)).sum::<u64>(),
            30 * 30 + 10 * 10
        );
    }

    #[test]
    fn rows_that_meet_or_overlap_become_one_band() {
        let mut bands = Vec::new();
        rows_of(
            &[
                Rect::new(Point::new(0, 40), Size::new(10, 10)),
                Rect::new(Point::new(80, 10), Size::new(10, 10)),
                // Starts exactly where the first one ends, so the two are one copy.
                Rect::new(Point::new(40, 50), Size::new(10, 6)),
            ],
            &mut bands,
        );
        assert_eq!(bands, vec![10..20, 40..56]);
    }
}
