//! The rows of an order node whose entries are ordered top to bottom that meet the draw damage.

use core::ops::Range;

use iceblink::DeviceRect;
use iceblink::damage::MAX_DAMAGE;
use smallvec::SmallVec;

use crate::frame::walk::Walk;

/// Ranges of rows in ascending order, none of which meets another.
pub(super) type Bands = SmallVec<[Range<usize>; MAX_DAMAGE]>;

impl Walk<'_> {
    /// Fills `bands` with the rows of the order node at `slot` that meet the draw damage, and
    /// reports whether the node's entries are ordered top to bottom.
    ///
    /// A row meets the damage when the vertical extent of a damage rectangle that meets `ink`, the
    /// node's device ink, reaches it. Inside filtered groups the output and the damage are grown
    /// by what the filters read. Entries outside the filled rows miss the damage.
    ///
    /// Nothing is filled when the entries are not ordered.
    pub(super) fn rows(&self, slot: u32, ink: DeviceRect, bands: &mut Bands) -> bool {
        let Some(record) = self.scene.orders.at(slot) else {
            return false;
        };
        let Some(rows) = record.rows.as_ref() else {
            return false;
        };
        let carry = record.ink.carry();
        let travel = carry.map_or(0, |carry| self.scene.travel(carry.scroll)[1]);
        let meeting = |band: DeviceRect| {
            let top = band.origin.y.saturating_sub(travel);
            let bottom = top.saturating_add(band.size.height);
            rows.meeting(top, bottom)
        };

        if self.draw.is_full() {
            let visible = self
                .frame_bounds()
                .outset(iceblink::geom::Edges::uniform(self.reach));
            bands.push(meeting(
                carry.map_or(visible, |carry| self.scene.still_pixels(carry.still)),
            ));
            return true;
        }

        let cuts = || self.reached().filter_map(|rect| rect.intersection(ink));
        let Some(union) = cuts().reduce(|union, cut| union.union(cut)) else {
            return true;
        };

        let whole = meeting(union);
        // Two searches per rectangle cost about as much as walking past two rows.
        if whole.len() <= 2 * self.draw.len() {
            bands.push(whole);
            return true;
        }

        bands.extend(cuts().map(meeting).filter(|band| !band.is_empty()));
        merge(bands);
        true
    }
}

/// Sorts `bands`, which are not empty, and joins the ranges that meet or touch each other.
fn merge(bands: &mut Bands) {
    if bands.len() < 2 {
        return;
    }

    bands.sort_unstable_by_key(|band| band.start);
    let mut last = 0;
    for at in 1..bands.len() {
        let band = bands[at].clone();
        if band.start <= bands[last].end {
            bands[last].end = bands[last].end.max(band.end);
        } else {
            last += 1;
            bands[last] = band;
        }
    }
    bands.truncate(last + 1);
}
