//! A filter chain as the renderer executes it: blurs, drop shadows and folded colour matrices.

use iceblink::FrameFilter;

use crate::frame::filter::matrix::ColorMatrix;

/// One step of a chain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Step {
    Matrix(ColorMatrix),
    Blur(f32),
    DropShadow {
        offset: [f32; 2],
        deviation: f32,
        color: [f32; 4],
    },
}

/// The steps of a filter chain.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Chain {
    steps: Vec<Step>,
}

impl Chain {
    /// Returns the chain of `filters`, with consecutive colour matrices folded into one and
    /// identities dropped.
    ///
    /// A shader filter is the identity, since application shaders cannot run here.
    pub(crate) fn of(filters: &[FrameFilter]) -> Self {
        let mut steps: Vec<Step> = Vec::new();
        for filter in filters {
            if let Some(matrix) = ColorMatrix::of(*filter) {
                match steps.last_mut() {
                    Some(Step::Matrix(held)) => *held = held.then(matrix),
                    _ => steps.push(Step::Matrix(matrix)),
                }
                continue;
            }
            match *filter {
                // A blur of zero is the identity.
                FrameFilter::Blur(deviation) if deviation.is_nan() || deviation <= 0.0 => {}
                FrameFilter::Blur(deviation) => steps.push(Step::Blur(deviation)),
                FrameFilter::DropShadow {
                    offset,
                    std_dev,
                    color,
                } => steps.push(Step::DropShadow {
                    offset,
                    deviation: std_dev.max(0.0),
                    color: color.0,
                }),
                _ => {}
            }
        }
        steps.retain(|step| !matches!(step, Step::Matrix(matrix) if matrix.is_identity()));
        Self { steps }
    }

    /// Returns the steps before a trailing colour matrix, and that matrix, which the composite
    /// applies.
    pub(crate) fn split(&self) -> (&[Step], ColorMatrix) {
        match self.steps.last() {
            Some(Step::Matrix(matrix)) => (&self.steps[..self.steps.len() - 1], *matrix),
            _ => (&self.steps, ColorMatrix::identity()),
        }
    }
}
