//! Counts of the work the compositor derives from its retained state.

/// Counts of derived work since the compositor was created.
///
/// Every count only grows, so the difference of two readings is the work between them.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Work {
    /// Chunks lowered into frame kinds.
    pub(crate) chunks_lowered: u64,
    /// Lowerings whose atlas content was checked against the scene in full.
    pub(crate) lowerings_checked: u64,
    /// Chunks whose local orders were computed.
    pub(crate) local_orders: u64,
    /// Chunks whose item inks were computed for a commit.
    pub(crate) inks_computed: u64,
    /// Entries of order nodes whose inks were summed again.
    pub(crate) entries_summed: u64,
    /// Item inks of scrolled content measured for the damage of a refused shift.
    pub(crate) moving_inks: u64,
    /// Chunks with glyph runs whose glyphs a commit looked up for rasterisation.
    pub(crate) glyph_chunks_prefetched: u64,
    /// Commits that removed an object, whose references were counted to check it is unused.
    pub(crate) use_checks: u64,
    /// Chunks whose bytes a commit counted again.
    pub(crate) bytes_counted: u64,
    /// Users of changed clips checked one by one for whether they keep their pixels.
    pub(crate) clip_users_checked: u64,
    /// Walks over the paint order that routing a gesture made.
    pub(crate) routing_walks: u64,
    /// Carried chunks and order nodes whose carry a commit replaced, keeping their inks.
    pub(crate) carries_replaced: u64,
}

impl Work {
    /// Calls `out` with every count and its stable name.
    pub(crate) fn report(&self, out: &mut dyn FnMut(&'static str, u64)) {
        out("work.chunks_lowered", self.chunks_lowered);
        out("work.lowerings_checked", self.lowerings_checked);
        out("work.local_orders", self.local_orders);
        out("work.inks_computed", self.inks_computed);
        out("work.entries_summed", self.entries_summed);
        out("work.moving_inks", self.moving_inks);
        out("work.glyph_chunks_prefetched", self.glyph_chunks_prefetched);
        out("work.use_checks", self.use_checks);
        out("work.bytes_counted", self.bytes_counted);
        out("work.clip_users_checked", self.clip_users_checked);
        out("work.routing_walks", self.routing_walks);
        out("work.carries_replaced", self.carries_replaced);
    }
}
