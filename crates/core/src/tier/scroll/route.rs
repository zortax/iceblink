//! The scroll nodes a gesture at a device point may move.

use iceblink::{Anchor, ScrollId};
use smallvec::SmallVec;

use crate::tier::View;
use crate::tier::scroll::state::{Geometry, behaviour};

/// Returns the scroll nodes that apply to the transform node `space`, innermost first, that the
/// compositor scrolls and whose port holds the device point `position`.
///
/// The walk goes from `space` towards the root and stops above a fixed node, since no offset
/// above it applies.
pub(crate) fn candidates(
    view: &View<'_>,
    space: u32,
    position: [f32; 2],
) -> SmallVec<[ScrollId; 4]> {
    let mut chain = SmallVec::new();
    let mut current = Some(space);
    let mut steps = 0;
    while let Some(slot) = current {
        let Some(record) = view.transforms.at(slot) else {
            break;
        };
        if let Some(scroll) = record.node.scroll
            && let Some(node) = view.scrolls.get(scroll.slot(), scroll.generation())
            && behaviour(node).is_some()
            && let Some(geometry) = Geometry::of(view, node)
            && geometry.holds(position)
        {
            chain.push(scroll);
        }

        if matches!(record.node.anchor, Anchor::Fixed) {
            break;
        }
        current = record.node.parent.map(|parent| parent.slot());
        steps += 1;
        if steps > view.transforms.span() {
            break;
        }
    }
    chain
}
