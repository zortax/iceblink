//! The pass being accumulated.

use iceblink::DeviceRect;

use crate::plan::order::DrawOrder;
use crate::plan::pass::cells::InkCells;
use crate::plan::pass::overlap::Intervening;

/// One path admitted to a pass.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OpenPath {
    /// The path's index in `Frame::paths`.
    pub(crate) index: u32,
    pub(crate) clip: u32,
    pub(crate) order: DrawOrder,
    /// The pixels the path's outline covers before any clip.
    pub(crate) reach: DeviceRect,
}

/// A pass being accumulated, kept between frames to reuse its allocations.
#[derive(Debug, Default)]
pub(crate) struct Open {
    pub(crate) active: bool,
    pub(crate) paths: Vec<OpenPath>,
    /// The paths' inks, in the same order.
    pub(crate) inks: Vec<DeviceRect>,
    pub(crate) cells: InkCells,
    /// Every drawn non-path instance since the pass's first path.
    pub(crate) intervening: Vec<Intervening>,
    /// Whether an intervening instance already splits the pass at its next path, for the readings
    /// that ignore the candidate.
    pub(crate) caught: bool,
    /// The highest order among the paths, where the composite belongs.
    // The highest and not the last: order follows overlap rather than emission, so a later path
    // can take a lower order than an earlier one, and a composite below any path of its pass
    // buries that path under the backgrounds it was painted over.
    pub(crate) composite_order: DrawOrder,
    /// Whether the pass admits no further path, because one of its paths is drawn through a mask.
    pub(crate) sealed: bool,
    /// Whether the pass is drawn into a group's target.
    pub(crate) nested: bool,
}

impl Open {
    /// Starts an empty pass.
    pub(crate) fn start(&mut self, nested: bool) {
        self.active = true;
        self.paths.clear();
        self.inks.clear();
        self.cells.clear();
        self.intervening.clear();
        self.caught = false;
        self.composite_order = 0;
        self.sealed = false;
        self.nested = nested;
    }

    /// Admits a path of ink `ink`.
    pub(crate) fn admit(&mut self, path: OpenPath, ink: DeviceRect) {
        self.paths.push(path);
        self.cells.insert(self.inks.len(), ink);
        self.inks.push(ink);
        self.composite_order = self.composite_order.max(path.order);
    }
}
