//! What a frame that breaks the frame contract is reported as.

use core::fmt;

/// A rule of the frame contract that a frame broke, with where it broke it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameViolation {
    /// The rule.
    pub rule: Rule,
    /// Where the frame breaks the rule and the values involved.
    pub detail: String,
}

impl FrameViolation {
    pub(crate) fn new(rule: Rule, detail: impl Into<String>) -> Self {
        Self {
            rule,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for FrameViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.rule, self.detail)
    }
}

impl std::error::Error for FrameViolation {}

/// The rules a frame is checked against.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rule {
    /// A damage rectangle or a shift region lies outside the output, or a shift's exposed bands
    /// lie outside the draw damage.
    Damage,
    /// A page operation refers to a page in the wrong state, or an upload reaches outside its
    /// page or past its bytes.
    PageOps,
    /// A shader operation creates a live shader slot or destroys one that is not live.
    ShaderOps,
    /// A chunk note inserts a resident or retired revision, or retires one that is not resident.
    ChunkNotes,
    /// A lane source or a drawn chunk refers to no instance or no resident revision.
    Sources,
    /// A frame slot refers to no table entry, shader or page.
    Slots,
    /// A paint reference's kind differs from the paint in its slot.
    PaintKinds,
    /// An enumeration, a bit set or a colour holds a value outside its listed ones, or a paint
    /// reference holds an inline colour.
    Values,
    /// A sprite tile or a marks page is on a page of another format, or reaches outside its page.
    Tiles,
    /// A marks instance refers to no payload, or reads past it.
    MarkPayloads,
    /// A batch range reaches outside its lane, or a batch of a kind drawn one at a time has a
    /// range.
    BatchRanges,
    /// The batches of a lane do not draw each of its sources once, in lane order.
    BatchCoverage,
    /// An instance of a sprite or shader-quad batch has another page or binding than its batch.
    BatchBindings,
    /// An external, backdrop, group or vector pass is drawn by no batch or by several, or a batch
    /// refers to one that does not exist.
    BatchObjects,
    /// Group markers do not pair up and nest.
    GroupNesting,
    /// A group's bounds leave the output or the source of the group around it, or its source
    /// does not hold them.
    GroupBounds,
    /// The vector plan breaks one of its rules.
    VectorPlan,
    /// The mark plan breaks one of its rules.
    MarkPlan,
    /// An opaque cover refers to no opaque quad of a quads batch outside every group, or the frame
    /// has more than four.
    OpaqueCovers,
    /// A glyph sprite under a translation lands off whole device pixels or is resampled.
    WholePixels,
    /// The frame holds something the renderer's capabilities rule out.
    Capabilities,
}

/// Renders `violations` one per line, for a panic message.
pub fn describe(violations: &[FrameViolation]) -> String {
    let mut text = String::new();
    for violation in violations {
        text.push_str(&violation.to_string());
        text.push('\n');
    }
    text
}
