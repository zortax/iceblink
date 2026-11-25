//! What a frame's rasterisation cost and could not do.

/// What the rounds of one frame cost and could not do.
///
/// Every round a frame prepares counts, including a round prepared again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rasterised {
    /// Clip outlines applied inside the scratch, for residual clips and path clips.
    pub clip_layers: u32,
    /// Paths left undrawn because a residual clip link has no outline.
    pub unclippable: u32,
    /// Paths left undrawn because neither their fill nor their stroke paints anything.
    pub unpaintable: u32,
    /// Line segments the frame tested samples against.
    pub segments: u32,
}
