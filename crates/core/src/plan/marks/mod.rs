//! The mark plan: where union marks sum their coverage.
//!
//! Each drawn union mark gets a bin, a region of a single-channel scratch page the size of the
//! output. Bins are placed in rows across at most [`MAX_MARK_PAGES`] pages in paint order, and the
//! marks that find no room take turns at the origin of one spare page.
//!
//! A bin holds the mark's ink cut to the output and to the bounding box of the draw damage
//! rectangles the ink meets. The bin keeps the mark's whole ink on the output when the mark is
//! inside a group, or when the frame holds a backdrop whose source covers pixels outside what it
//! writes.

mod shelves;

use iceblink::damage::DamageSet;
use iceblink::{DeviceRect, MAX_MARK_PAGES, MarkBin, MarkPlan};

use self::shelves::Shelves;
use crate::plan::pixels;

/// A drawn union mark, as the bin plan reads it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct UnionMark {
    /// The mark's push index in the marks lane.
    pub(crate) push: u32,
    pub(crate) ink: DeviceRect,
    /// Whether the mark is drawn inside a group.
    pub(crate) nested: bool,
}

/// What the bin plan of a frame reads besides the marks.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame<'a> {
    pub(crate) size: [u32; 2],
    pub(crate) damage: &'a DamageSet,
    /// Whether a backdrop's source covers pixels outside what it writes.
    pub(crate) wide_backdrop: bool,
}

/// Plans one bin per mark of `marks`, which are in paint order, into `plan`.
///
/// `positions` maps a mark's push index to its position in the marks lane's `sources`.
pub(crate) fn plan(marks: &[UnionMark], positions: &[u32], frame: Frame<'_>, plan: &mut MarkPlan) {
    plan.clear();
    plan.full_damage = frame.damage.is_full();
    if marks.is_empty() {
        return;
    }

    let output = pixels::output(frame.size);
    let cut = !frame.damage.is_full() && !frame.wide_backdrop;
    let mut shelves = Shelves::new(frame.size);
    for mark in marks {
        let Some(mut region) = mark.ink.intersection(output) else {
            continue;
        };
        if cut
            && !mark.nested
            && let Some(within) = touched(frame.damage, region)
        {
            region = within;
        }

        let size = [region.size.width as u32, region.size.height as u32];
        let (page, at) = shelves.place(size).unwrap_or((MAX_MARK_PAGES, [0, 0]));
        plan.pages = plan.pages.max(page + 1);
        plan.extent = [
            plan.extent[0].max(at[0] + size[0]),
            plan.extent[1].max(at[1] + size[1]),
        ];

        let bin = MarkBin {
            item: positions[mark.push as usize],
            region,
            at,
            page,
        };
        if page < MAX_MARK_PAGES {
            plan.bins.push(bin);
        } else {
            plan.overflow.push(bin);
        }
    }

    plan.bins.sort_unstable_by_key(|bin| bin.item);
    plan.overflow.sort_unstable_by_key(|bin| bin.item);
}

/// Returns the part of `region` inside the bounding box of the damage rectangles it meets.
fn touched(damage: &DamageSet, region: DeviceRect) -> Option<DeviceRect> {
    damage
        .rects()
        .iter()
        .filter(|rect| rect.intersects(region))
        .copied()
        .reduce(DeviceRect::union)
        .and_then(|touched| touched.intersection(region))
}
