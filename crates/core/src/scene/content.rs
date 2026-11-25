//! The transform nodes and chunks in the content of a scroll node.

use iceblink::Anchor;
use smallvec::SmallVec;

use crate::scene::Scene;
use crate::scene::record::TransformRecord;
use crate::scene::slab::Slab;

/// Returns the slots of the scroll nodes that apply to the transform node `space`, up to the
/// first fixed node.
pub(crate) fn applied_scrolls(
    transforms: &Slab<TransformRecord>,
    space: u32,
) -> SmallVec<[u32; 4]> {
    let mut chain = SmallVec::new();
    let mut current = Some(space);
    let mut steps = 0;

    while let Some(slot) = current {
        let Some(record) = transforms.at(slot) else {
            break;
        };
        if let Some(scroll) = record.node.scroll {
            chain.push(scroll.slot());
        }
        if matches!(record.node.anchor, Anchor::Fixed) || steps > transforms.span() {
            break;
        }
        current = record.node.parent.map(|parent| parent.slot());
        steps += 1;
    }

    chain
}

/// Reports whether a chunk refers to a transform node that the scroll node at `scroll` applies
/// to.
///
/// The answer ignores whether the chunk is in the order tree, and counts a chunk whose props or
/// local clips alone refer to such a node.
pub(crate) fn draws_in(scene: &Scene, scroll: u32) -> bool {
    let Some(record) = scene.scrolls.at(scroll) else {
        return false;
    };

    let transforms = &scene.transforms;
    let mut stack: SmallVec<[u32; 8]> = record
        .dependents
        .iter()
        .copied()
        .filter(|&node| {
            transforms
                .at(node)
                .and_then(|transform| transform.node.scroll)
                .is_some_and(|id| id.slot() == scroll)
        })
        .collect();

    let mut visited = 0;
    while let Some(slot) = stack.pop() {
        let Some(transform) = transforms.at(slot) else {
            continue;
        };
        if !transform.links.users.is_empty() {
            return true;
        }

        // A fixed node applies no scroll node above it.
        stack.extend(transform.children.iter().copied().filter(|&child| {
            transforms
                .at(child)
                .is_some_and(|child| !matches!(child.node.anchor, Anchor::Fixed))
        }));
        visited += 1;
        if visited > transforms.span() {
            break;
        }
    }
    false
}
