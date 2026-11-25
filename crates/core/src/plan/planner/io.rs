//! What a plan reads and where it writes.

use iceblink::damage::DamageSet;
use iceblink::smallvec::SmallVec;
use iceblink::{Batch, Frame, Lanes, MarkPlan, OpaqueCover, VectorPlan};

use crate::plan::emission::Emission;
use crate::plan::pass::Overlap;

/// What one frame's plan reads.
#[derive(Clone, Copy, Debug)]
pub struct Input<'a> {
    /// The frame's emission stream.
    pub emission: &'a Emission,
    /// The draw damage: the pixels the renderer redraws.
    ///
    /// A full set plans the frame whole, as for a target of unknown age.
    pub damage: &'a DamageSet,
    /// The output's size, in device pixels.
    pub size: [u32; 2],
    /// The reading of the rule that starts a new vector pass.
    pub overlap: Overlap,
    /// Whether vector passes keep their whole region, for a renderer that keeps rasters by key.
    pub uncut: bool,
}

impl<'a> Input<'a> {
    /// Returns the input of `emission` drawn into `damage` on an output of `size`, with the
    /// default [`Overlap`] and passes cut to the damage.
    pub fn new(emission: &'a Emission, damage: &'a DamageSet, size: [u32; 2]) -> Self {
        Self {
            emission,
            damage,
            size,
            overlap: Overlap::default(),
            uncut: false,
        }
    }
}

/// The parts of a frame a plan writes.
///
/// The fields borrow one frame's parts separately, so the frame's tables stay free for the
/// [`ClipChains`](crate::plan::ClipChains) the plan extends.
#[derive(Debug)]
pub struct Output<'a> {
    /// The lanes, of which the plan writes every `sources`.
    pub lanes: &'a mut Lanes,
    /// The draw sequence.
    pub batches: &'a mut Vec<Batch>,
    /// The vector passes.
    pub vector_plan: &'a mut VectorPlan,
    /// The bins of union marks.
    pub mark_plan: &'a mut MarkPlan,
    /// The opaque covers.
    pub opaque_covers: &'a mut SmallVec<[OpaqueCover; 4]>,
}

impl<'a> Output<'a> {
    /// Returns the parts of `frame` a plan writes.
    pub fn of(frame: &'a mut Frame) -> Self {
        Self {
            lanes: &mut frame.lanes,
            batches: &mut frame.batches,
            vector_plan: &mut frame.vector_plan,
            mark_plan: &mut frame.mark_plan,
            opaque_covers: &mut frame.opaque_covers,
        }
    }
}
