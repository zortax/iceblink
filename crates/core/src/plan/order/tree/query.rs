// DERIVED-FROM: the GPUI project, crates/gpui/src/bounds_tree.rs (Apache-2.0)
// Adapted as described in `order/tree/mod.rs`.

//! The overlap query of the bounds tree.

use iceblink::DeviceRect;

use crate::plan::order::DrawOrder;
use crate::plan::order::tree::BoundsTree;
use crate::plan::order::tree::node::NodeKind;

impl BoundsTree {
    /// Returns the highest order among the leaves `query` intersects, or zero when it intersects
    /// none.
    pub(super) fn max_intersecting(&mut self, query: DeviceRect) -> DrawOrder {
        let Some(root) = self.root else { return 0 };

        // The highest leaf answers most queries outright, because interface content is mostly
        // stacked rather than scattered.
        if let Some(leaf) = self.max_leaf {
            let node = &self.nodes[leaf as usize];
            if query.intersects(node.bounds) {
                return node.max_order;
            }
        }

        self.stack.clear();
        self.stack.push(root);
        let mut found = 0;
        while let Some(index) = self.stack.pop() {
            let node = &self.nodes[index as usize];
            if node.max_order <= found || !query.intersects(node.bounds) {
                continue;
            }
            match &node.kind {
                NodeKind::Leaf => found = found.max(node.max_order),
                NodeKind::Internal { children } => {
                    // The highest-order child is last, so it pops first and prunes the most.
                    self.stack.extend(
                        children
                            .iter()
                            .copied()
                            .filter(|child| self.nodes[*child as usize].max_order > found),
                    );
                }
            }
        }
        found
    }
}
