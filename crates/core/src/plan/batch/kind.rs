//! The kinds a frame draws, in their tie-break order.

use crate::plan::emission::LaneKind;

/// A kind of thing a frame draws.
///
/// The variant order is the order of kinds at equal draw order. Instances at equal draw order do
/// not overlap, so the variant order affects batching only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DrawKind {
    /// The start of a group.
    GroupBegin,
    /// A shadow.
    Shadow,
    /// A quad.
    Quad,
    /// A shader quad.
    ShaderQuad,
    /// Marks.
    Marks,
    /// The composite of a vector pass.
    Path,
    /// A line.
    Line,
    /// A mono sprite.
    Mono,
    /// A subpixel sprite.
    Subpixel,
    /// A colour sprite.
    Color,
    /// An external quad.
    External,
    /// A backdrop.
    Backdrop,
    /// The end of a group.
    GroupEnd,
}

impl DrawKind {
    /// Every kind, in tie-break order.
    pub const ALL: [Self; 13] = [
        Self::GroupBegin,
        Self::Shadow,
        Self::Quad,
        Self::ShaderQuad,
        Self::Marks,
        Self::Path,
        Self::Line,
        Self::Mono,
        Self::Subpixel,
        Self::Color,
        Self::External,
        Self::Backdrop,
        Self::GroupEnd,
    ];

    /// Returns the kind of the instances of `lane`.
    pub const fn of_lane(lane: LaneKind) -> Self {
        match lane {
            LaneKind::Shadows => Self::Shadow,
            LaneKind::Quads => Self::Quad,
            LaneKind::ShaderQuads => Self::ShaderQuad,
            LaneKind::Marks => Self::Marks,
            LaneKind::Lines => Self::Line,
            LaneKind::Mono => Self::Mono,
            LaneKind::Subpixel => Self::Subpixel,
            LaneKind::Color => Self::Color,
        }
    }
}
