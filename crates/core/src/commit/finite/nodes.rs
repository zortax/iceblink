//! Property nodes, filter chains and shader parameter blocks.

use iceblink::{
    Anchor, ClipShape, EffectNode, Filter, Matrix4, ParamsBlock, ScrollNode, TransformNode,
};

use super::all;

/// Reports whether the matrix and the anchor geometry of `node` are finite.
///
/// A sticky inset may also be NaN, which leaves its edge free.
pub(crate) fn transform(node: &TransformNode) -> bool {
    let local = matrix(&node.local);
    let anchor = match &node.anchor {
        Anchor::Sticky(sticky) => {
            all(sticky.rect.iter().chain(&sticky.limit))
                && sticky.inset.iter().all(|inset| !inset.is_infinite())
        }
        Anchor::Thumb(thumb) => all(thumb.track.iter()),
        Anchor::Scrolls | Anchor::Fixed => true,
    };
    local && anchor
}

/// Reports whether the port, the content size, the offset and the snap points of `node` are
/// finite, and its source sets hold only listed sources.
///
/// An axis of a snap point may also be NaN, which does not snap.
pub(crate) fn scroll(node: &ScrollNode) -> bool {
    let snap = node.behaviour.as_ref().is_none_or(|behaviour| {
        let snaps = behaviour
            .snap
            .iter()
            .flat_map(|point| point.offset)
            .all(|axis| !axis.is_infinite());
        snaps && behaviour.bounce.is_valid() && behaviour.chain.is_valid()
    });
    all(node.port.iter().chain(&node.content).chain(&node.offset)) && snap
}

/// Reports whether the rectangle, radii, corner exponents or mask source of `shape` are finite,
/// and its mask mode is listed.
pub(crate) fn clip_shape(shape: &ClipShape) -> bool {
    match shape {
        ClipShape::RoundedRect {
            rect,
            radii,
            shapes,
        } => all(rect.iter().chain(radii).chain(shapes)),
        ClipShape::Mask {
            rect, source, mode, ..
        } => all(rect.iter().chain(source)) && mode.is_valid(),
    }
}

/// Reports whether every entry of `matrix` is finite.
pub(crate) fn matrix(matrix: &Matrix4) -> bool {
    matrix.columns.iter().all(all)
}

/// Reports whether the opacity and the filters of `node` are well formed and its blend mode is
/// listed.
pub(crate) fn effect(node: &EffectNode) -> bool {
    node.opacity.is_finite() && node.blend.is_valid() && filters(&node.filters)
}

/// Reports whether every number of every filter is finite and every colour valid.
pub(crate) fn filters(filters: &[Filter]) -> bool {
    filters.iter().all(|filter| match filter {
        Filter::Blur(value) | Filter::Opacity(value) => value.is_finite(),
        Filter::DropShadow {
            offset,
            std_dev,
            color,
        } => all(offset.iter().chain([std_dev])) && super::color(color),
        Filter::ColorMatrix(matrix) => all(matrix),
        Filter::Shader { reach, .. } => reach.is_finite(),
    })
}

/// Reports whether the pointer fields of `block` are finite.
pub(crate) fn params(block: &ParamsBlock) -> bool {
    all(block.pointer.iter().chain([&block.hovered]))
}
