//! The diagnostic of a frame that plans more vector passes than expected.

use iceblink::VectorPlan;

/// A frame that planned at least [`PassWarning::THRESHOLD`] vector passes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PassWarning {
    /// How many passes were planned.
    pub passes: usize,
    /// The count at which the warning fires.
    pub threshold: usize,
}

impl PassWarning {
    /// The pass count at which a frame that redraws part of the output is considered mis-planned.
    // Three is the most a frame of interleaved vector content is expected to need once damage has
    // culled it.
    pub const THRESHOLD: usize = 4;

    /// Returns the warning for `plan`, or `None` below the threshold or when `full_repaint` says
    /// the frame redraws the whole output.
    pub fn of(plan: &VectorPlan, full_repaint: bool) -> Option<Self> {
        (!full_repaint && plan.passes.len() >= Self::THRESHOLD).then_some(Self {
            passes: plan.passes.len(),
            threshold: Self::THRESHOLD,
        })
    }
}

impl core::fmt::Display for PassWarning {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "planned {} vector passes, at or over the threshold of {}: on a damaged frame this is \
             a coalescing defect",
            self.passes, self.threshold
        )
    }
}
