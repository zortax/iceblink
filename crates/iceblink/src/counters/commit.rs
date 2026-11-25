//! The counters of one commit.

/// The counters of one commit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommitCounters {
    /// Item and arena bytes recorded.
    pub bytes_recorded: u64,
    /// Chunks recorded.
    pub chunks_recorded: u32,
    /// Recorded chunks identical to the chunk they replaced.
    pub chunks_identical: u32,
    /// Chunks removed.
    pub chunks_removed: u32,
    /// `set_offset` commands applied.
    pub offsets_written: u32,
    /// Transform, scroll, clip, effect and order-effect writes applied, creates excluded.
    pub property_writes: u32,
    /// `set_paint` and `set_params` commands applied.
    pub paint_writes: u32,
    /// Order entries inserted or removed by splices.
    pub keys_spliced: u32,
    /// Glyph bitmaps rasterised.
    pub glyphs_rasterised: u32,
    /// `write_image` commands applied.
    pub images_written: u32,
}
