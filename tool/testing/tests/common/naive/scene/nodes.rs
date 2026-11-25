//! The transform and scroll nodes of the scene.

use iceblink::{
    Anchor, Matrix4, ScrollId, ScrollNode, StickyAnchor, Transaction, TransformId, TransformNode,
};

use crate::common::naive::root_node;

/// The nodes chunk 1 of the scene draws under.
pub(super) struct Nodes {
    /// A node scrolled 30 pixels down a tall port.
    pub(super) scrolled: TransformId,
    /// A node under `scrolled` that sticks to the top of the port.
    pub(super) sticky: TransformId,
    /// A scaled node under `scrolled` that ignores the scroll offset.
    pub(super) fixed: TransformId,
    /// A node under the root, rotated and moved.
    pub(super) turned: TransformId,
}

/// Creates the nodes under `root`.
pub(super) fn create(transaction: &mut Transaction, root: TransformId) -> Nodes {
    let scroll = transaction.create_scroll(ScrollNode {
        space: root,
        port: [0.0, 0.0, 96.0, 64.0],
        content: [96.0, 400.0],
        offset: [0.0, 30.0],
        behaviour: None,
    });
    let scrolled = transaction.create_transform(TransformNode {
        parent: Some(root),
        scroll: Some(scroll),
        ..root_node()
    });
    let sticky = transaction.create_transform(TransformNode {
        parent: Some(scrolled),
        anchor: Anchor::Sticky(sticky(scroll)),
        ..root_node()
    });
    let fixed = transaction.create_transform(TransformNode {
        parent: Some(scrolled),
        anchor: Anchor::Fixed,
        local: Matrix4::scale(1.5, 1.5, 1.0),
        ..root_node()
    });
    let turned = transaction.create_transform(TransformNode {
        parent: Some(root),
        local: Matrix4::from_columns([
            [0.8, 0.6, 0.0, 0.0],
            [-0.6, 0.8, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [40.0, 4.0, 0.0, 1.0],
        ]),
        ..root_node()
    });

    Nodes {
        scrolled,
        sticky,
        fixed,
        turned,
    }
}

/// An anchor that holds a band at the top edge of `scroll`'s port.
fn sticky(scroll: ScrollId) -> StickyAnchor {
    StickyAnchor {
        scroll,
        rect: [0.0, 10.0, 96.0, 8.0],
        limit: [0.0, 0.0, 96.0, 300.0],
        inset: [0.0, f32::NAN, f32::NAN, f32::NAN],
    }
}
