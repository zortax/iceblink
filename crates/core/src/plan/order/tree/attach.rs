// DERIVED-FROM: the GPUI project, crates/gpui/src/bounds_tree.rs (Apache-2.0)
// Adapted as described in `order/tree/mod.rs`.

//! Filing leaves into the bounds tree.

use iceblink::DeviceRect;

use crate::plan::order::DrawOrder;
use crate::plan::order::tree::BoundsTree;
use crate::plan::order::tree::node::{
    MAX_CHILDREN, Node, NodeKind, merged_half_perimeter, ordered_pair,
};

impl BoundsTree {
    /// Files a leaf for `bounds` at `order` and returns its index.
    pub(super) fn insert_leaf(&mut self, bounds: DeviceRect, order: DrawOrder) -> u32 {
        let leaf = self.push_node(Node {
            bounds,
            max_order: order,
            kind: NodeKind::Leaf,
        });

        let Some(root) = self.root else {
            self.root = Some(leaf);
            return leaf;
        };

        if matches!(self.nodes[root as usize].kind, NodeKind::Leaf) {
            let root_bounds = self.nodes[root as usize].bounds;
            let root_order = self.nodes[root as usize].max_order;
            let children = ordered_pair(root, root_order, leaf, order);
            let new_root = self.push_node(Node {
                bounds: root_bounds.union(bounds),
                max_order: root_order.max(order),
                kind: NodeKind::Internal { children },
            });
            self.root = Some(new_root);
            return leaf;
        }

        self.descend_and_attach(root, leaf, bounds, order);
        self.propagate(bounds, order);
        leaf
    }

    /// Walks down from `from` and attaches `leaf` where the tree grows least.
    ///
    /// The descent resumes part-way down when the previous one reached a node that contains
    /// `bounds`.
    fn descend_and_attach(&mut self, from: u32, leaf: u32, bounds: DeviceRect, order: DrawOrder) {
        let mut current = self.resume(bounds).unwrap_or(from);
        loop {
            self.path.push(current);
            let NodeKind::Internal { children } = &self.nodes[current as usize].kind else {
                unreachable!("descent only visits internal nodes");
            };
            // A plain scan rather than a `min_by`: this is the hottest loop of the tree. The first
            // of several equal candidates wins, as with `min_by`.
            let mut candidates = children.iter().copied().enumerate();
            let (mut best_position, mut best) =
                candidates.next().expect("an internal node has children");
            let mut best_cost = merged_half_perimeter(bounds, self.nodes[best as usize].bounds);
            for (position, child) in candidates {
                let cost = merged_half_perimeter(bounds, self.nodes[child as usize].bounds);
                if cost < best_cost {
                    best_cost = cost;
                    best = child;
                    best_position = position;
                }
            }

            if matches!(self.nodes[best as usize].kind, NodeKind::Internal { .. }) {
                current = best;
                continue;
            }

            let room = match &self.nodes[current as usize].kind {
                NodeKind::Internal { children } => children.len() < MAX_CHILDREN,
                NodeKind::Leaf => false,
            };
            if room {
                self.attach_child(current, leaf, bounds, order);
            } else {
                self.split_against(current, best, best_position, leaf, bounds, order);
            }
            return;
        }
    }

    /// Returns the deepest node of the previous descent that contains `bounds`, and truncates the
    /// path to that node's ancestors.
    // Consecutive inserts are nearly always neighbours on the screen, so the previous descent
    // usually already reached the right subtree. The node a rectangle is filed under never changes
    // the order it gets, because the query prunes only on facts `propagate` keeps true along the
    // whole path, which is why the ancestors stay on it.
    fn resume(&mut self, bounds: DeviceRect) -> Option<u32> {
        for index in (0..self.path.len()).rev() {
            let node = self.path[index];
            // A path holds interior nodes and never leaves.
            if self.nodes[node as usize].bounds.contains_rect(bounds) {
                self.path.truncate(index);
                return Some(node);
            }
        }
        self.path.clear();
        None
    }

    /// Adds `leaf` to `parent`, keeping the highest-order child last.
    fn attach_child(&mut self, parent: u32, leaf: u32, bounds: DeviceRect, order: DrawOrder) {
        let node = &mut self.nodes[parent as usize];
        let was_max = node.max_order;
        if let NodeKind::Internal { children } = &mut node.kind {
            children.push(leaf);
            if order <= was_max {
                let last = children.len() - 1;
                children.swap(last - 1, last);
            }
        }
        node.bounds = node.bounds.union(bounds);
        node.max_order = was_max.max(order);
    }

    /// Pairs `leaf` with the full node's best child under a new internal node.
    fn split_against(
        &mut self,
        parent: u32,
        sibling: u32,
        sibling_position: usize,
        leaf: u32,
        bounds: DeviceRect,
        order: DrawOrder,
    ) {
        let sibling_bounds = self.nodes[sibling as usize].bounds;
        let sibling_order = self.nodes[sibling as usize].max_order;
        let children = ordered_pair(sibling, sibling_order, leaf, order);
        let merged_order = sibling_order.max(order);
        let internal = self.push_node(Node {
            bounds: sibling_bounds.union(bounds),
            max_order: merged_order,
            kind: NodeKind::Internal { children },
        });

        let node = &mut self.nodes[parent as usize];
        let parent_max = node.max_order;
        if let NodeKind::Internal { children } = &mut node.kind {
            let last = children.len() - 1;
            children[sibling_position] = internal;
            if merged_order > parent_max {
                children.swap(sibling_position, last);
            }
        }
        node.bounds = node.bounds.union(bounds);
        node.max_order = parent_max.max(order);
    }

    /// Widens bounds and raises orders back up the descent path.
    fn propagate(&mut self, bounds: DeviceRect, order: DrawOrder) {
        let mut raised: Option<u32> = None;
        for index in (0..self.path.len()).rev() {
            let node_index = self.path[index];
            let node = &mut self.nodes[node_index as usize];
            node.bounds = node.bounds.union(bounds);
            if node.max_order < order {
                node.max_order = order;
                if let (Some(child), NodeKind::Internal { children }) = (raised, &mut node.kind)
                    && let Some(position) = children.iter().position(|held| *held == child)
                {
                    let last = children.len() - 1;
                    children.swap(position, last);
                }
            }
            raised = Some(node_index);
        }
    }
}
