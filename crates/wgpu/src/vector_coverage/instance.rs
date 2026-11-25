//! The records the coverage shader reads.

use bytemuck::{Pod, Zeroable};

/// One outline to fill.
///
/// Positions and extents are in the pixels of the scratch layer the outline is drawn into.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct Item {
    /// The quad to draw, as origin and extent.
    pub(crate) bounds: [f32; 4],
    /// The extent of the whole scratch layer, which maps the quad into clip space.
    pub(crate) viewport: [f32; 4],
    /// The straight, gamma-encoded colour.
    pub(crate) color: [f32; 4],
    /// The first band, the band count, 1 for the even-odd rule, and the first clip run.
    pub(crate) control: [f32; 4],
    /// The clip run count, the top of the first band, the band height, and an unused lane.
    pub(crate) bands: [f32; 4],
}

/// Where the outline of one clip lies in the outline table.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct Run {
    /// The first segment, the segment count, 1 for the even-odd rule, and an unused lane.
    pub(crate) span: [f32; 4],
}

impl Run {
    /// Returns a run of `count` segments from `first`, tested by the even-odd rule if `even_odd`
    /// is set and by the non-zero rule otherwise.
    pub(crate) fn new(first: usize, count: usize, even_odd: bool) -> Self {
        Self {
            span: [
                first as f32,
                count as f32,
                f32::from(u8::from(even_odd)),
                0.0,
            ],
        }
    }
}
