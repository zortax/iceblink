//! The vector plan: which paths each vector pass rasterises and where it is composited.

use core::ops::Range;

use crate::geometry::DeviceRect;

/// The vector passes of a frame.
///
/// A pass rasterises a group of paths into scratch memory, and one composite, its `Paths` batch,
/// draws the result into the target at the pass's position in paint order. The plan satisfies
/// these invariants:
///
/// 1. Every drawn path appears in exactly one pass, and the frame has exactly one `Paths` batch
///    per pass.
/// 2. Every instance that precedes a path of a pass in paint order and intersects its ink is drawn
///    before the pass's composite, and every instance that follows it and intersects its ink is
///    drawn after.
/// 3. A path's full clip chain admits exactly what the pass clip and the path's residual admit
///    together. A residual holds no `Mask` link.
/// 4. A pass is `instanced` only when no two of its paths have intersecting inks.
/// 5. A pass's region contains the part of each path's ink that lies in the draw damage.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VectorPlan {
    /// Every planned path, grouped by pass, in paint order within each pass.
    pub items: Vec<PlannedPath>,
    /// The passes, in the order of their `Paths` batches.
    pub passes: Vec<PlannedPass>,
}

impl VectorPlan {
    /// Reports whether the frame has no pass.
    pub fn is_empty(&self) -> bool {
        self.passes.is_empty()
    }

    /// Returns the paths of `pass`.
    pub fn items_of(&self, pass: &PlannedPass) -> &[PlannedPath] {
        &self.items[pass.items.start as usize..pass.items.end as usize]
    }

    /// Empties the plan, keeping the allocations.
    pub fn clear(&mut self) {
        self.items.clear();
        self.passes.clear();
    }
}

/// One path of a vector pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlannedPath {
    /// The path's index in `Frame::paths`.
    pub path: u32,
    /// The frame clip slot of the part of the path's chain the pass clip does not cover, applied
    /// inside the raster, or `FRAME_NO_CLIP`.
    pub residual: u32,
    /// The frame clip slot a per-path composite binds.
    pub clip: u32,
    /// The path's ink, relative to the pass region's origin.
    pub ink: DeviceRect,
}

/// One vector pass.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedPass {
    /// The pass's paths, as a range of `VectorPlan::items`.
    pub items: Range<u32>,
    /// The device region rasterised, aligned outwards to multiples of 16 and cut to the output.
    pub region: DeviceRect,
    /// The frame clip slot a whole-pass composite binds.
    pub clip: u32,
    /// Whether the pass may be composited one path at a time, each composite reading only its
    /// path's ink and binding its path's `clip`.
    pub instanced: bool,
    /// Whether the region was cut to the draw damage, so paths reach past it.
    pub clamped: bool,
    /// Whether a path's outline reaches past the region, at the output's edge or past the ink its
    /// clip leaves.
    pub overhangs: bool,
    /// What the pass's raster depends on, or `PassKey::NONE`.
    pub key: PassKey,
}

/// A digest of everything a vector pass's raster depends on.
///
/// Two passes with the same key other than `NONE` and regions of the same size rasterise to the
/// same pixels, so a rasteriser may keep a raster and composite it again for a later pass with
/// the key. A key is issued only for a pass that is not clamped and whose paths have no residual
/// clip. The key is derived from the addresses of the `Arc`s of the paths' outlines, so a
/// rasteriser that keeps a raster holds those `Arc`s for as long as it keeps the raster.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PassKey(pub u64);

impl PassKey {
    /// No key: the raster is drawn each time the pass is planned.
    pub const NONE: Self = Self(0);
}
