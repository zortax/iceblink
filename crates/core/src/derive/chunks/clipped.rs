//! Where a chunk the commit left alone can draw differently because a clip changed.

use iceblink::ChunkKey;

use crate::derive::chunks::KeyChanges;
use crate::derive::clips::ClipChanges;
use crate::derive::world::WorldChanges;
use crate::geometry::coverage::CoverageChange;
use crate::scene::Scene;
use crate::scene::kind::Kind;

/// Returns the device pixels where the clip nodes chunk `key` refers to cover differently after
/// the commit.
///
/// The change is the union of the coverage changes of those nodes' chains. It is every pixel when
/// the chunk is gone, when the commit also changed the chunk itself, or when it moved a space the
/// chunk refers to.
pub(super) fn clip_change(
    scene: &Scene,
    key: ChunkKey,
    changes: Option<&KeyChanges>,
    world: &WorldChanges,
    clips: &ClipChanges,
) -> CoverageChange {
    let Some(record) = scene.chunks.get(&key) else {
        return CoverageChange::whole();
    };
    if changes.is_some() {
        return CoverageChange::whole();
    }
    let mut change = CoverageChange::default();
    for object in &record.derived.deps {
        match object.kind {
            Kind::Transform if world.moved.contains_key(&object.slot) => {
                return CoverageChange::whole();
            }
            Kind::Clip => {
                if let Some(coverage) = clips.coverage.get(&object.slot) {
                    change.add(coverage);
                }
            }
            _ => {}
        }
    }
    change
}
