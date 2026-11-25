//! What a compositor supports.

/// What a compositor supports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompositorCapabilities {
    /// Whether frames are built on a thread the compositor owns.
    pub threaded: bool,
    /// Whether `Transaction::animate` is honoured.
    pub animations: bool,
    /// Whether `scroll_input` routes gestures and moves scroll nodes.
    pub scroll_input: bool,
    /// Whether `hit_test` returns hits.
    pub hit_test: bool,
    /// Whether damage and cost are bounded, so an identical re-recording adds no damage and
    /// commits and frames cost in proportion to what changed.
    pub precise: bool,
}
