//! The transform nodes a commit starts or stops moving.

use iceblink::Matrix4;
use rustc_hash::FxHashSet;

use crate::commit::journal::{Journal, Op};
use crate::content::stretches;
use crate::derive::world::WorldChanges;
use crate::scene::Scene;

/// Starts or stops the motion of every transform node whose world matrix changed its linear part
/// in the commit:
///
/// - If glyph tiles under the new matrix are stretched while it moves, the node starts moving.
/// - Otherwise the node stops moving.
///
/// Nodes the commit created are left alone.
pub(crate) fn start(scene: &mut Scene, journal: &Journal, world: &WorldChanges) {
    let mut created: Option<FxHashSet<u32>> = None;
    for moved in world.moved.values() {
        if same_linear(&moved.before, &moved.after) {
            continue;
        }
        let created = created.get_or_insert_with(|| {
            journal
                .transforms
                .iter()
                .filter(|change| matches!(change.op, Op::Created(_)))
                .map(|change| change.slot)
                .collect()
        });
        if created.contains(&moved.slot) {
            continue;
        }
        let Some(record) = scene.transforms.at_mut(moved.slot) else {
            continue;
        };
        record.moving = stretches(&moved.after);
        if record.moving {
            scene.motion.start(moved.slot);
        } else {
            scene.motion.stop(moved.slot);
        }
    }
}

/// Reports whether two matrices have the same linear part, or are equal when either is not
/// two-dimensional.
fn same_linear(before: &Matrix4, after: &Matrix4) -> bool {
    let (Some(before), Some(after)) = (before.to_affine2(), after.to_affine2()) else {
        return before == after;
    };
    [before.a, before.b, before.c, before.d] == [after.a, after.b, after.c, after.d]
}
