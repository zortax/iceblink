// DERIVED-FROM: the GPUI project, crates/gpui/src/bounds_tree.rs (Apache-2.0)
// The algorithm here is adapted from that work, licensed under the Apache License, Version 2.0.
// It is an R-tree whose nodes cache the highest draw order in their subtree, queried for "the
// greatest order among everything this rectangle intersects" with pruning on both the cached order
// and the bounding box. The global-maximum fast path, the highest-order-child-last invariant, the
// insert-above-all barrier and the order floor come from it too. It has been modified to work over
// whole device pixels, to address nodes by index rather than by raw pointer, to count its
// insertions, and to re-enter the previous insert's descent path rather than restart every descent
// at the root.

//! The R-tree that assigns draw order.

mod attach;
mod node;
mod query;

use iceblink::DeviceRect;

use crate::plan::order::DrawOrder;
use crate::plan::order::tree::node::{Node, NodeKind};

/// An R-tree of device rectangles that gives each inserted rectangle the lowest draw order above
/// everything it overlaps.
///
/// Two rectangles given equal order never overlap, so instances at one order may be drawn in any
/// sequence. Disjoint rectangles share low orders.
///
/// Two barriers apply on top of the overlap rule. [`BoundsTree::insert_above_all`] takes one more
/// than the highest order assigned, whatever it overlaps. [`BoundsTree::set_order_floor`] raises
/// the lowest order every later insert takes.
///
/// Rectangles are half-open, so rectangles that only share an edge do not overlap.
///
/// ```
/// use iceblink_core::plan::BoundsTree;
/// use iceblink::DeviceRect;
/// use iceblink::geom::{Point, Size};
///
/// let rect = |x: i32| DeviceRect::new(Point::new(x, 0), Size::new(10, 10));
///
/// let mut tree = BoundsTree::new();
/// assert_eq!(tree.insert(rect(0)), 1);
/// assert_eq!(tree.insert(rect(5)), 2);
/// assert_eq!(tree.insert(rect(100)), 1);
/// ```
#[derive(Clone, Debug, Default)]
pub struct BoundsTree {
    nodes: Vec<Node>,
    root: Option<u32>,
    /// The leaf holding the highest order.
    max_leaf: Option<u32>,
    order_floor: DrawOrder,
    /// The previous insert's descent, which the next insert resumes from.
    path: Vec<u32>,
    /// The query's search stack, kept to reuse its allocation.
    stack: Vec<u32>,
    inserts: u32,
}

impl BoundsTree {
    /// Creates an empty tree.
    pub fn new() -> Self {
        Self::default()
    }

    /// Empties the tree, drops its order floor and resets the insert count, keeping the
    /// allocations.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.root = None;
        self.max_leaf = None;
        self.order_floor = 0;
        self.path.clear();
        self.stack.clear();
        self.inserts = 0;
    }

    /// Returns how many rectangles have been inserted since the tree was created or cleared.
    pub fn inserts(&self) -> u32 {
        self.inserts
    }

    /// Returns how many leaves the tree holds.
    pub fn len(&self) -> usize {
        self.nodes
            .iter()
            .filter(|node| matches!(node.kind, NodeKind::Leaf))
            .count()
    }

    /// Reports whether nothing has been inserted.
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// Returns the highest order assigned, or zero when nothing has been inserted.
    pub fn max_order(&self) -> DrawOrder {
        self.max_leaf
            .map_or(0, |leaf| self.nodes[leaf as usize].max_order)
    }

    /// Returns the lowest order the next insert may take.
    pub fn order_floor(&self) -> DrawOrder {
        self.order_floor
    }

    /// Raises the lowest order every later insert takes to `floor`.
    ///
    /// A floor below the current one changes nothing. Overlapping inserts still step above one
    /// another above the floor.
    pub fn set_order_floor(&mut self, floor: DrawOrder) {
        self.order_floor = self.order_floor.max(floor);
    }

    /// Inserts `bounds` and returns one more than the highest order it overlaps, at least the
    /// order floor.
    pub fn insert(&mut self, bounds: DeviceRect) -> DrawOrder {
        let order = (self.max_intersecting(bounds) + 1).max(self.order_floor);
        self.file(bounds, order);
        order
    }

    /// Reserves `span + 1` consecutive orders for instances already ordered among themselves and
    /// returns the lowest of them.
    ///
    /// The lowest order is the one [`BoundsTree::insert`] gives `bounds`. One leaf is filed for
    /// `bounds` at the highest reserved order, so a later insert that overlaps any part of `bounds`
    /// sorts above every reserved order.
    ///
    /// The orders stay sound as long as the instances were ordered against each other by this rule
    /// and lie inside `bounds`.
    pub fn insert_block(&mut self, bounds: DeviceRect, span: DrawOrder) -> DrawOrder {
        let base = (self.max_intersecting(bounds) + 1).max(self.order_floor);
        self.file(bounds, base.saturating_add(span));
        base
    }

    /// Inserts `bounds` one above the highest order assigned, whatever it overlaps, and returns
    /// that order.
    pub fn insert_above_all(&mut self, bounds: DeviceRect) -> DrawOrder {
        let order = self.max_order() + 1;
        let leaf = self.insert_leaf(bounds, order);
        self.max_leaf = Some(leaf);
        self.inserts += 1;
        order
    }

    /// Files `bounds` at `order`, so every later insert that overlaps it sorts above `order`.
    pub(crate) fn raise(&mut self, bounds: DeviceRect, order: DrawOrder) {
        self.file(bounds, order);
    }

    /// Files a leaf for `bounds` at `order` and keeps the highest leaf current.
    fn file(&mut self, bounds: DeviceRect, order: DrawOrder) {
        let leaf = self.insert_leaf(bounds, order);
        self.max_leaf = match self.max_leaf {
            Some(previous) if self.nodes[previous as usize].max_order >= order => Some(previous),
            _ => Some(leaf),
        };
        self.inserts += 1;
    }

    /// Appends a node and returns its index.
    fn push_node(&mut self, node: Node) -> u32 {
        self.nodes.push(node);
        (self.nodes.len() - 1) as u32
    }
}
