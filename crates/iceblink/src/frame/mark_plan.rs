//! The mark plan: where the coverage of union marks is summed.

use crate::geometry::DeviceRect;

/// The most scratch pages the bins of one frame use, before the spare page.
pub const MAX_MARK_PAGES: u32 = 8;

/// The bins of a frame's union marks.
///
/// A `Marks` instance with `UNION` sums the coverage of its prims into a bin, a region of a
/// single-channel scratch page, and its composite paints the instance through the bin. Every
/// union instance drawn has exactly one bin in `bins` or `overflow`, and no other instance has
/// one.
///
/// A bin contains the part of the instance's ink on the output that lies in the draw damage, or
/// its whole ink on the output when `full_damage` is set. A bin may hold more of the ink, up to
/// all of it on the output, such as for an instance inside a group. The renderer clears a bin
/// before it writes coverage into it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MarkPlan {
    /// The bins on the first `MAX_MARK_PAGES` pages, which do not overlap, sorted by `item`.
    pub bins: Vec<MarkBin>,
    /// The bins of the union instances that found no page, sorted by `item`.
    ///
    /// Each lies at the origin of page `MAX_MARK_PAGES`, the spare page, and the renderer writes
    /// it just before its own composite.
    pub overflow: Vec<MarkBin>,
    /// One more than the highest page a bin lies on.
    pub pages: u32,
    /// The size of every page, at most the output's size.
    ///
    /// Every bin fits inside it from its `at`.
    pub extent: [u32; 2],
    /// Whether the plan was made for a frame that redraws every pixel.
    pub full_damage: bool,
}

impl MarkPlan {
    /// Returns the bin in `bins` of the instance at position `item` of the marks lane.
    pub fn bin(&self, item: u32) -> Option<&MarkBin> {
        find(&self.bins, item)
    }

    /// Returns the bin in `overflow` of the instance at position `item` of the marks lane.
    pub fn overflow_bin(&self, item: u32) -> Option<&MarkBin> {
        find(&self.overflow, item)
    }

    /// Reports whether no instance has a bin.
    pub fn is_empty(&self) -> bool {
        self.bins.is_empty() && self.overflow.is_empty()
    }

    /// Empties the plan, keeping the allocations.
    pub fn clear(&mut self) {
        self.bins.clear();
        self.overflow.clear();
        self.pages = 0;
        self.extent = [0, 0];
        self.full_damage = false;
    }
}

/// Returns the bin of `item` in `bins`, which are sorted by item.
fn find(bins: &[MarkBin], item: u32) -> Option<&MarkBin> {
    bins.binary_search_by_key(&item, |bin| bin.item)
        .ok()
        .map(|at| &bins[at])
}

/// The scratch region one union mark sums its coverage into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkBin {
    /// The instance's position in the marks lane, counting instances.
    pub item: u32,
    /// The device pixels the bin holds.
    pub region: DeviceRect,
    /// Where the region's origin lies in its page, in texels.
    pub at: [u32; 2],
    /// The scratch page the bin lies on.
    pub page: u32,
}
