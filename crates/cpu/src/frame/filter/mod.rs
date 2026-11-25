//! Executing a filter chain over a group's or a backdrop's canvas, and compositing the result.
//!
//! Each step writes a region grown by the reach of the steps after it, so the last step's output
//! is whole. A trailing colour matrix is applied by the composite.

pub(crate) mod blur;
mod chain;
mod composite;
mod matrix;

use iceblink::geom::{Device, Edges, Rect};

use crate::frame::pool::Pool;
use crate::target::{Canvas, Precision};

pub(crate) use self::chain::Chain;
pub(crate) use self::composite::{Composite, composite};

use self::chain::Step;
use self::matrix::ColorMatrix;

/// What a blur reads outside the pixels its source holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EdgeMode {
    /// Transparent black.
    Transparent,
    /// The nearest pixel the source holds.
    Duplicate,
}

/// A drop shadow drawn beneath filtered content: the content's alpha blurred, tinted and moved.
#[derive(Debug)]
pub(crate) struct Shadow {
    pub(crate) blurred: Canvas,
    pub(crate) color: [f32; 4],
    pub(crate) offset: [f32; 2],
    /// What the composite reads outside `blurred`.
    pub(crate) edge: EdgeMode,
}

/// The result of a filter chain: the content, the matrix the composite applies to it, and the
/// shadows drawn beneath it.
#[derive(Debug)]
pub(crate) struct Filtered {
    pub(crate) content: Canvas,
    pub(crate) matrix: ColorMatrix,
    pub(crate) shadows: Vec<Shadow>,
}

impl Filtered {
    /// Returns every canvas to `pool`.
    pub(crate) fn release(self, pool: &mut Pool) {
        pool.release(self.content);
        for shadow in self.shadows {
            pool.release(shadow.blurred);
        }
    }
}

/// Runs `chain` over `source`, which holds `valid`, so that the result is whole over `output`.
///
/// Blurs and drop shadows read outside `valid` as `edge` says.
pub(crate) fn run(
    pool: &mut Pool,
    chain: &Chain,
    source: Canvas,
    output: Rect<i32, Device>,
    valid: Rect<i32, Device>,
    edge: EdgeMode,
) -> Filtered {
    let (steps, folded) = chain.split();
    let regions = staging(steps, output, valid);
    let mut filtered = Filtered {
        content: source,
        matrix: folded,
        shadows: Vec::new(),
    };

    // What the canvas the next step reads holds, which a drop shadow leaves alone.
    let mut holds = valid;
    for (step, writes) in steps.iter().zip(regions) {
        match *step {
            Step::Matrix(matrix) => {
                let next = transformed(pool, &filtered.content, matrix, writes);
                let previous = std::mem::replace(&mut filtered.content, next);
                pool.release(previous);
                holds = writes;
            }
            Step::Blur(deviation) => {
                let blurred = blur::blur(
                    pool,
                    &filtered.content.texture(),
                    writes,
                    holds,
                    deviation,
                    edge,
                );
                let previous = std::mem::replace(&mut filtered.content, blurred);
                pool.release(previous);
                holds = writes;
            }
            Step::DropShadow {
                offset,
                deviation,
                color,
            } => {
                // A shadow is drawn beneath the content, so the chain's content stays as it is.
                let blurred = blur::blur(
                    pool,
                    &filtered.content.texture(),
                    writes,
                    holds,
                    deviation,
                    edge,
                );
                filtered.shadows.push(Shadow {
                    blurred,
                    color,
                    offset,
                    edge,
                });
            }
        }
    }
    filtered
}

// Returns `content` through `matrix` over `writes`.
fn transformed(
    pool: &mut Pool,
    content: &Canvas,
    matrix: ColorMatrix,
    writes: Rect<i32, Device>,
) -> Canvas {
    let mut next = pool.acquire(writes, Precision::Float, false);
    let texture = content.texture();
    let mut surface = next.surface();
    let mut row = Vec::with_capacity(writes.width().max(0) as usize);
    for y in writes.top()..writes.bottom() {
        row.clear();
        row.extend(
            (writes.left()..writes.right())
                .map(|x| matrix.apply(texture.sample([x as f32 + 0.5, y as f32 + 0.5]))),
        );
        surface.replace_row(writes.left(), y, &row);
    }
    next
}

// The region each step writes: the output grown by the reach of the steps after it, within
// what the source holds.
fn staging(
    steps: &[Step],
    output: Rect<i32, Device>,
    valid: Rect<i32, Device>,
) -> Vec<Rect<i32, Device>> {
    let mut regions = Vec::with_capacity(steps.len());
    let mut beyond = 0;
    for step in steps.iter().rev() {
        regions.push(
            output
                .outset(Edges::uniform(beyond))
                .intersection(valid)
                .unwrap_or(output),
        );
        beyond += reach_of(step);
    }
    regions.reverse();
    regions
}

fn reach_of(step: &Step) -> i32 {
    match step {
        Step::Matrix(_) => 0,
        Step::Blur(deviation) => blur::reach(*deviation),
        Step::DropShadow {
            offset, deviation, ..
        } => blur::reach(*deviation) + offset[0].abs().max(offset[1].abs()).ceil() as i32,
    }
}
