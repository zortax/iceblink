//! The device bounds of clip chains, split by whether each link follows a scroll offset.
//!
//! The two parts of a split intersect to the chain's bounds as the `clip` module computes them.

use iceblink::{ClipRef, LocalParent, RecordedChunk};
use smallvec::SmallVec;

use crate::geometry::bounds::DeviceBox;
use crate::geometry::clip::{link_box, shape_rect};
use crate::geometry::coverage::Partial;
use crate::scene::Scene;
use crate::scene::follows::{ClipSplit, Follows, StillRoot};

/// Returns the split of the chain of clip node `slot`, from its link and its parent's stored
/// split.
pub(crate) fn node_split(scene: &Scene, slot: u32) -> ClipSplit {
    let Some(record) = scene.clips.at(slot) else {
        return ClipSplit::OPEN;
    };
    let parent = record
        .node
        .parent
        .and_then(|parent| scene.clips.at(parent.slot()))
        .map_or(ClipSplit::OPEN, |parent| parent.split);

    match scene.transforms.at(record.node.space.slot()) {
        Some(space) => {
            let shape = &record.node.shape;
            let bounds = link_box(&space.world, shape, shape_rect(shape));
            let partial = Partial::of_link(&space.world, shape);
            parent.with(StillRoot::Node(slot), space.follows, bounds, partial)
        }
        None => parent,
    }
}

/// Returns the split of every chunk-local clip of `chunk` at `offset`.
pub(crate) fn local_splits(
    scene: &Scene,
    chunk: &RecordedChunk,
    offset: [f32; 2],
) -> SmallVec<[ClipSplit; 2]> {
    let space = scene.transforms.at(chunk.props.space.slot());
    let follows = space.map_or(Follows::Still, |space| space.follows);
    let chunk_clip = node_of(scene, chunk.props.clip.map(|clip| clip.slot()));

    let mut splits: SmallVec<[ClipSplit; 2]> = SmallVec::new();
    for clip in &chunk.local_clips {
        let parent = match clip.parent {
            LocalParent::Chunk => chunk_clip,
            LocalParent::Local(index) => splits
                .get(index as usize)
                .copied()
                .unwrap_or(ClipSplit::OPEN),
            LocalParent::Node(id) => node_of(scene, Some(id.slot())),
            LocalParent::None => ClipSplit::OPEN,
        };

        let mut rect = shape_rect(&clip.shape);
        rect[0] += offset[0];
        rect[1] += offset[1];
        let own = space.map_or(DeviceBox::EVERYWHERE, |space| {
            link_box(&space.world, &clip.shape, rect)
        });
        let partial = space.and_then(|space| Partial::of_link(&space.world, &clip.shape));
        splits.push(parent.with(StillRoot::Local, follows, own, partial));
    }

    splits
}

/// Returns the split of the chain an item's clip reference draws it through.
pub(crate) fn item_split(
    scene: &Scene,
    chunk: &RecordedChunk,
    locals: &[ClipSplit],
    clip: ClipRef,
) -> ClipSplit {
    if clip == ClipRef::NONE {
        return ClipSplit::OPEN;
    }
    if clip == ClipRef::INHERIT {
        return node_of(scene, chunk.props.clip.map(|clip| clip.slot()));
    }
    if let Some(index) = clip.local_index() {
        return locals
            .get(index as usize)
            .copied()
            .unwrap_or(ClipSplit::OPEN);
    }
    node_of(scene, Some(clip.0))
}

/// Returns the stored split of clip node `slot`, or that of a chain without links.
fn node_of(scene: &Scene, slot: Option<u32>) -> ClipSplit {
    slot.and_then(|slot| scene.clips.at(slot))
        .map_or(ClipSplit::OPEN, |clip| clip.split)
}
