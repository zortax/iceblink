//! Writing a finished pass into the vector plan.

use iceblink::geom::{Point, Size};
use iceblink::{DeviceRect, FRAME_NO_CLIP, PassKey, PlannedPass, PlannedPath, VectorPlan};

use crate::plan::order::DrawOrder;
use crate::plan::pass::cells::InkCells;
use crate::plan::pass::clips::{self, ClipChains};
use crate::plan::pass::coalesce::open::OpenPath;
use crate::plan::pass::region;

/// What every recorded pass of a frame shares.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub(crate) size: [u32; 2],
    /// The rectangle a pass outside every group is cut to, if the frame is cut.
    pub(crate) cut: Option<DeviceRect>,
}

/// Scratch space and running totals of the recording.
#[derive(Debug, Default)]
pub(crate) struct Recorder {
    /// The composite order of each pass, parallel to the plan's passes.
    pub(crate) orders: Vec<DrawOrder>,
    /// Clip links the residuals add inside rasters.
    pub(crate) clip_layers: u32,
    covered: Vec<DeviceRect>,
    cells: InkCells,
}

impl Recorder {
    pub(crate) fn clear(&mut self) {
        self.orders.clear();
        self.clip_layers = 0;
    }

    /// Records one pass of `paths`, whose inks are `inks`, into `out`.
    ///
    /// The pass clip is the deepest chain all paths share, and each path's residual is the rest of
    /// its chain. A pass outside every group is cut to the frame's cut. A pass whose region is
    /// empty is not recorded.
    pub(crate) fn record(
        &mut self,
        paths: &[OpenPath],
        inks: &[DeviceRect],
        nested: bool,
        frame: Frame,
        clips: &mut impl ClipChains,
        out: &mut VectorPlan,
    ) {
        let Some(bounds) = inks.iter().copied().reduce(DeviceRect::union) else {
            return;
        };

        let whole = region::aligned(bounds, frame.size);
        let region = match frame.cut {
            Some(cut) if !nested => whole
                .intersection(region::aligned(cut, frame.size))
                .unwrap_or(DeviceRect::ZERO),
            _ => whole,
        };
        if region.is_empty() {
            return;
        }
        let clamped = region != whole;
        let overhangs = paths.iter().any(|path| !whole.contains_rect(path.reach));

        let pass_clip = paths
            .iter()
            .map(|path| path.clip)
            .reduce(|left, right| clips::common_ancestor(clips, left, right))
            .unwrap_or(FRAME_NO_CLIP);

        let first = out.items.len() as u32;
        self.covered.clear();
        for (path, ink) in paths.iter().zip(inks) {
            let residual = clips::residual(clips, path.clip, pass_clip);
            self.clip_layers += clips::depth(clips, residual);
            let ink = cut(*ink, region);
            self.covered.push(ink);
            out.items.push(PlannedPath {
                path: path.index,
                residual,
                clip: path.clip,
                ink: relative(ink, region),
            });
        }

        let composite = paths.iter().map(|path| path.order).max().unwrap_or(0);
        out.passes.push(PlannedPass {
            items: first..out.items.len() as u32,
            region,
            clip: pass_clip,
            instanced: pairwise_disjoint(&self.covered, &mut self.cells),
            clamped,
            overhangs,
            key: PassKey::NONE,
        });
        self.orders.push(composite);
    }
}

/// Returns the part of `ink` inside `region`, or an empty rectangle at the region's corner.
pub(crate) fn cut(ink: DeviceRect, region: DeviceRect) -> DeviceRect {
    ink.intersection(region)
        .unwrap_or_else(|| DeviceRect::new(region.origin, Size::new(0, 0)))
}

/// Returns `ink` measured from the origin of `region`.
pub(crate) fn relative(ink: DeviceRect, region: DeviceRect) -> DeviceRect {
    DeviceRect::new(
        Point::new(ink.left() - region.left(), ink.top() - region.top()),
        ink.size,
    )
}

/// Reports whether no two of `inks` overlap.
///
/// `cells` is scratch space.
pub(crate) fn pairwise_disjoint(inks: &[DeviceRect], cells: &mut InkCells) -> bool {
    // A few inks compare faster pair by pair than through the cells.
    const PAIRWISE: usize = 16;
    if inks.len() <= PAIRWISE {
        return inks.iter().enumerate().all(|(index, left)| {
            inks[index + 1..]
                .iter()
                .all(|right| !left.intersects(*right))
        });
    }

    cells.clear();
    for (index, ink) in inks.iter().enumerate() {
        if cells.meets(*ink, inks, |_| true) {
            return false;
        }
        cells.insert(index, *ink);
    }
    true
}
