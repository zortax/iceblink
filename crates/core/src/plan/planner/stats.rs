//! The counts one frame's planning reports.

/// The counts of one planned frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlanStats {
    /// Instances, paths, external quads and backdrops drawn.
    pub drawn: u32,
    /// Instances, paths and external quads left out because their ink missed the output or the
    /// draw damage.
    pub culled: u32,
    /// Paths among `culled`.
    pub paths_culled: u32,
    /// Insertions into the bounds tree.
    pub order_inserts: u32,
    /// Groups drawn.
    pub groups: u32,
    /// Batches in the draw sequence.
    pub batches: u32,
    /// Passes in the vector plan.
    pub vector_passes: u32,
    /// Clip links the residuals of the vector plan apply inside rasters.
    pub clip_layers: u32,
    /// Bins in the mark plan, overflow included.
    pub mark_bins: u32,
}
