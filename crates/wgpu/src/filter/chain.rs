//! A filter chain as the renderer executes it: blurs, shadows, shaders and folded matrices.

use iceblink::FrameFilter;

use crate::filter::matrix::ColorMatrix;

/// One step of a filter chain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Step {
    /// A colour matrix applied to each pixel.
    Matrix(ColorMatrix),
    /// A blur with this standard deviation, in device pixels.
    Blur(f32),
    /// A filter shader, with its frame shader slot, its parameter slot and how far it reads.
    Custom {
        shader: u32,
        params: u32,
        reach: f32,
    },
    /// A drop shadow drawn behind the content.
    DropShadow {
        offset_x: f32,
        offset_y: f32,
        blur: f32,
        color: [f32; 4],
    },
}

/// The steps of a filter chain, in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Chain {
    steps: Vec<Step>,
}

impl Chain {
    /// Returns the chain of `filters`, with consecutive colour matrices folded into one and
    /// identities dropped.
    pub(crate) fn of(filters: &[FrameFilter]) -> Self {
        let mut steps: Vec<Step> = Vec::new();
        for filter in filters {
            match ColorMatrix::of(*filter) {
                Some(matrix) => match steps.last_mut() {
                    Some(Step::Matrix(held)) => *held = held.then(matrix),
                    _ => steps.push(Step::Matrix(matrix)),
                },
                None => steps.push(match filter {
                    // A blur of zero is the identity, and resampling it would read past its region.
                    FrameFilter::Blur(deviation) if deviation.is_nan() || *deviation <= 0.0 => {
                        continue;
                    }
                    FrameFilter::Blur(deviation) => Step::Blur(*deviation),
                    FrameFilter::DropShadow {
                        offset,
                        std_dev,
                        color,
                    } => Step::DropShadow {
                        offset_x: offset[0],
                        offset_y: offset[1],
                        blur: std_dev.max(0.0),
                        color: color.0,
                    },
                    FrameFilter::Shader {
                        shader,
                        params,
                        reach,
                    } => Step::Custom {
                        shader: *shader,
                        params: *params,
                        reach: reach.max(0.0),
                    },
                    FrameFilter::ColorMatrix(_) | FrameFilter::Opacity(_) => continue,
                }),
            }
        }
        steps.retain(|step| !matches!(step, Step::Matrix(matrix) if matrix.is_identity()));
        Self { steps }
    }

    /// Returns the steps before a trailing colour matrix, and that matrix or the identity.
    pub(crate) fn split(&self) -> (&[Step], ColorMatrix) {
        match self.steps.last() {
            Some(Step::Matrix(matrix)) => (&self.steps[..self.steps.len() - 1], *matrix),
            _ => (&self.steps, ColorMatrix::identity()),
        }
    }
}
