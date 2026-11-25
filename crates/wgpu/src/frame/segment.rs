//! The planned frame: passes over targets, the draws in them, and the encoder work between them.

use core::ops::Range;

use iceblink::Batch;
use iceblink::geom::{Device, Rect};

use crate::effect::EffectKey;
use crate::frame::target::TargetRef;
use crate::pipeline::marks::MarkKind;
use crate::vector::VectorTarget;

/// One draw of a planned pass.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PlannedDraw {
    /// Clears the scissor to transparent.
    Clear,
    /// An instanced batch of the frame.
    Batch(Batch),
    /// A blur step over `source`, which downsamples or blurs along one axis.
    Blur {
        source: TargetRef,
        params: u32,
        downsample: bool,
    },
    /// A filter shader over a group target.
    Effect {
        source: TargetRef,
        shader: EffectKey,
        params: u32,
        block: u32,
    },
    /// A composite of `source`.
    Composite { source: TargetRef, params: u32 },
    /// The external quad `index`.
    External { index: u32, params: u32 },
    /// One kind of mark prims of the marks item at chunk `position`.
    Marks {
        kind: MarkKind,
        coverage: bool,
        position: usize,
        block: u32,
        instances: [u32; 2],
        page: Option<u32>,
    },
    /// The composite of a union marks item's coverage bin.
    MarksComposite { block: u32 },
    /// The vector composite instances `first..first + count`, read from the rasteriser's
    /// `target`.
    Vector {
        target: VectorTarget,
        first: u32,
        count: u32,
    },
}

/// What a pass does with its target's texels when it opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PassLoad {
    /// The pass draws over what the target holds.
    Keep,
    /// The pass clears the target to transparent first.
    Discard,
}

/// A pass over one target, cut to a scissor.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlannedPass {
    pub(crate) target: TargetRef,
    pub(crate) load: PassLoad,
    pub(crate) scissor: Rect<i32, Device>,
    /// The offset of the globals block the pass binds.
    pub(crate) globals: u32,
    /// The pass's draws, as a range of the plan's draws.
    pub(crate) draws: Range<usize>,
}

/// Work recorded on the encoder between passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EncoderOp {
    /// Copies a region of one target into another at one resolution.
    Capture {
        source: TargetRef,
        destination: TargetRef,
        region: Rect<i32, Device>,
    },
}

/// One step of a planned frame.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Segment {
    Encoder(EncoderOp),
    Pass(PlannedPass),
    /// The vector rasteriser prepares this round before the segments that follow.
    Round(usize),
}

impl Segment {
    pub(crate) fn round(&self) -> Option<usize> {
        match self {
            Self::Round(round) => Some(*round),
            Self::Encoder(_) | Self::Pass(_) => None,
        }
    }
}
