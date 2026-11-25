//! Where one clip node's own link covers differently after a commit.

use iceblink::{ClipNode, Matrix4};

use crate::derive::world::WorldChanges;
use crate::geometry::coverage::{CoverageChange, PlacedLink, link_change};
use crate::scene::Scene;

/// Returns the device pixels where the link of clip node `slot` covers differently than before
/// the commit, ignoring its ancestors.
///
/// `written` is the node's value before the commit when the commit wrote it, `Some(None)` when
/// the commit created it, and `None` when the commit left it alone. A node that was created or
/// moved to another parent changes every pixel.
pub(super) fn own_change(
    scene: &Scene,
    slot: u32,
    written: Option<Option<&ClipNode>>,
    world: &WorldChanges,
) -> CoverageChange {
    let Some(record) = scene.clips.at(slot) else {
        return CoverageChange::whole();
    };
    let node = &record.node;
    let before = match written {
        Some(Some(before)) => before,
        Some(None) => return CoverageChange::whole(),
        None => node,
    };
    if before.parent != node.parent {
        return CoverageChange::whole();
    }
    let (Some(old), Some(new)) = (
        world_before(scene, world, before.space.slot()),
        scene
            .transforms
            .at(node.space.slot())
            .map(|space| space.world),
    ) else {
        return CoverageChange::whole();
    };
    link_change(
        PlacedLink {
            shape: &before.shape,
            world: &old,
        },
        PlacedLink {
            shape: &node.shape,
            world: &new,
        },
    )
}

/// Returns the world matrix transform node `slot` had before the commit.
fn world_before(scene: &Scene, world: &WorldChanges, slot: u32) -> Option<Matrix4> {
    match world.moved.get(&slot) {
        Some(moved) => Some(moved.before),
        None => scene.transforms.at(slot).map(|space| space.world),
    }
}
