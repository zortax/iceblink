//! The cases of the draw suite, one module per group of frame kinds.

pub(crate) mod blends;
pub(crate) mod blur_cuts;
pub(crate) mod blur_edges;
pub(crate) mod clips;
pub(crate) mod colours;
pub(crate) mod effects;
pub(crate) mod external;
mod gauss;
pub(crate) mod lines;
pub(crate) mod marks;
pub(crate) mod paints;
pub(crate) mod paths;
pub(crate) mod quads;
pub(crate) mod sequence;
pub(crate) mod shaders;
pub(crate) mod shadows;
pub(crate) mod sprites;

/// The rectangle of cell `index` of a three by three grid over the output.
pub(crate) fn cell(index: usize) -> [f32; 4] {
    let column = (index % 3) as f32;
    let row = (index / 3) as f32;
    [4.0 + column * 42.0, 4.0 + row * 42.0, 36.0, 36.0]
}
