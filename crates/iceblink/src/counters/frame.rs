//! The counters of one frame.

/// The counters of one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameCounters {
    /// Order nodes the frame entered.
    pub order_nodes_visited: u32,
    /// Order nodes the frame skipped because their ink missed the damage.
    pub order_nodes_skipped: u32,
    /// Chunks with at least one item whose ink meets the draw damage.
    pub chunks_drawn: u32,
    /// Instances in the frame's lanes and arrays.
    pub items_drawn: u32,
    /// Items of visited chunks, and planned instances and paths, left out because they missed
    /// the damage.
    pub items_culled: u32,
    /// Insertions into the structure that orders instances.
    pub order_inserts: u32,
    /// Groups drawn.
    pub groups: u32,
    /// Passes in the vector plan.
    pub vector_passes: u32,
    /// Bins in the mark plan, overflow included.
    pub mark_bins: u32,
    /// Pixels in the draw damage.
    pub damage_px: u64,
    /// Pixels moved by shifts.
    pub shifted_px: u64,
    /// Scroll changes answered with damage because the shift rule failed.
    pub shift_refusals: u32,
    /// Glyph runs meeting the draw damage that drew nothing because the compositor has no glyph
    /// rasteriser.
    pub glyph_runs_without_raster: u32,
}
