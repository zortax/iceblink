//! Memory a compositor holds, and the limits on what it can rebuild.

/// Memory a compositor holds, in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompositorMemory {
    /// Retained chunks.
    pub chunk_bytes: u64,
    /// The frame tables: world matrices, clip chains, paints and parameter blocks.
    pub table_bytes: u64,
    /// Atlas pages.
    pub atlas_bytes: u64,
    /// The atlas bytes of pinned entries, which eviction keeps.
    pub atlas_pinned_bytes: u64,
    /// Image pixels.
    pub image_bytes: u64,
}

/// Soft limits on the caches a compositor rebuilds on demand.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompositorBudgets {
    /// The atlas bytes above which unpinned atlas entries are evicted, or `None` for no limit.
    pub atlas_bytes: Option<u64>,
}
