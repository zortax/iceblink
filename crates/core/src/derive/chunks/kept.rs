//! Chunks whose pixels and inks survive the clip changes of a commit.

use crate::derive::carry::carrier;
use crate::derive::chunks::Changes;
use crate::derive::clips::ChainBefore;
use crate::geometry::bounds::DeviceBox;
use crate::geometry::coverage::CoverageChange;
use crate::scene::Scene;
use crate::scene::chunk::ChunkRecord;
use crate::scene::kind::Kind;
use crate::scene::record::ClipRecord;

/// Reports whether chunk `record`, which the commit did not write, draws the same pixels with
/// the same inks after the commit as before.
///
/// It does when all of these hold:
///
/// 1. No scroll node can carry the chunk, and it holds no backdrop that reads around it.
/// 2. No space the chunk refers to moved.
/// 3. Every changed clip it refers to cuts the chunk's reach to the same box as before, or to
///    nothing before and after, and the spaces of its chain follow the same scroll offsets.
/// 4. The pixels where those clips cover differently do not meet the whole pixels of the reach.
///
/// The reach is the device box that holds every item before clips cut it.
pub(super) fn keeps_pixels(scene: &Scene, record: &ChunkRecord, changes: &Changes<'_>) -> bool {
    let inks = &record.derived.inks;
    if inks.carry().is_some() || record.derived.reads_around || carrier(scene, record).is_some() {
        return false;
    }
    let reach = inks.reach();
    record.derived.deps.iter().all(|object| match object.kind {
        Kind::Transform => !changes.world.moved.contains_key(&object.slot),
        Kind::Clip => {
            let Some(before) = changes.clips.before.get(&object.slot) else {
                return true;
            };
            scene.clips.at(object.slot).is_some_and(|clip| {
                before.follows == clip.split.follows
                    && cuts_alike(
                        before,
                        clip,
                        changes.clips.coverage.get(&object.slot),
                        reach,
                    )
            })
        }
        _ => true,
    })
}

/// Reports whether changed clip `clip`, which was `before` before the commit and covers
/// differently in `coverage`, leaves the pixels of a chunk with reach `reach` alone, as conditions 3
/// and 4 of [`keeps_pixels`] say for one clip whose spaces follow the same offsets.
// Inlined into the walk over every user of a written clip.
#[inline]
pub(super) fn cuts_alike(
    before: &ChainBefore,
    clip: &ClipRecord,
    coverage: Option<&CoverageChange>,
    reach: Option<DeviceBox>,
) -> bool {
    let Some(reach) = reach else {
        return true;
    };
    let (old, new) = (reach.intersect(before.bounds), reach.intersect(clip.bounds));
    if old != new && !(old.is_empty() && new.is_empty()) {
        return false;
    }
    let Some(coverage) = coverage else {
        return true;
    };
    !coverage.meets(DeviceBox {
        x0: reach.x0.floor(),
        y0: reach.y0.floor(),
        x1: reach.x1.ceil(),
        y1: reach.y1.ceil(),
    })
}
