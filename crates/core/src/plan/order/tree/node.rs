// DERIVED-FROM: the GPUI project, crates/gpui/src/bounds_tree.rs (Apache-2.0)
// Adapted as described in `order/tree/mod.rs`.

//! Nodes of the bounds tree.

use iceblink::DeviceRect;
use smallvec::SmallVec;

use crate::plan::order::DrawOrder;

/// The most children an internal node holds before it splits.
// Wide rather than binary: a shallower tree touches fewer nodes per query, and a bounding-box test
// per child is cheap enough for the width to pay for itself.
pub(super) const MAX_CHILDREN: usize = 12;

/// The children of an internal node.
pub(super) type Children = SmallVec<[u32; MAX_CHILDREN]>;

/// What a node is.
#[derive(Clone, Debug)]
pub(super) enum NodeKind {
    /// One inserted rectangle.
    Leaf,
    /// Nodes kept with the highest-order child last.
    Internal { children: Children },
}

/// One node of the tree.
#[derive(Clone, Debug)]
pub(super) struct Node {
    /// The rectangle containing the node and everything below it.
    pub(super) bounds: DeviceRect,
    /// The highest draw order below the node.
    pub(super) max_order: DrawOrder,
    pub(super) kind: NodeKind,
}

/// Returns the two children with the higher-order one last.
pub(super) fn ordered_pair(
    first: u32,
    first_order: DrawOrder,
    second: u32,
    second_order: DrawOrder,
) -> Children {
    let mut children = SmallVec::new();
    if second_order > first_order {
        children.push(first);
        children.push(second);
    } else {
        children.push(second);
        children.push(first);
    }
    children
}

/// Returns half the perimeter of the smallest rectangle containing both, the growth cost a
/// descent minimises.
///
/// An empty rectangle contributes nothing.
// Measured without building the union, because this runs once per child per level of every
// insert, the hottest loop of the tree.
pub(super) fn merged_half_perimeter(one: DeviceRect, two: DeviceRect) -> i64 {
    if one.is_empty() {
        return half_perimeter(two);
    }
    if two.is_empty() {
        return half_perimeter(one);
    }
    let left = one.left().min(two.left());
    let top = one.top().min(two.top());
    let right = one.right().max(two.right());
    let bottom = one.bottom().max(two.bottom());
    (i64::from(right) - i64::from(left)) + (i64::from(bottom) - i64::from(top))
}

/// Returns half a rectangle's perimeter.
fn half_perimeter(rect: DeviceRect) -> i64 {
    i64::from(rect.size.width) + i64::from(rect.size.height)
}
