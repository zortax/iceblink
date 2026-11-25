//! The translations that scroll offset changes move content by.

use iceblink::{ChunkKey, Matrix4};

use crate::derive::Derive;
use crate::derive::world::WorldChanges;
use crate::scene::kind::Kind;

/// Returns the translation that takes `before` to `after`, when the two differ by one.
pub(super) fn translation_between(before: &Matrix4, after: &Matrix4) -> Option<[f32; 2]> {
    let (a, b) = (before.columns, after.columns);
    let linear_same =
        a[0] == b[0] && a[1] == b[1] && a[2] == b[2] && a[3][2] == b[3][2] && a[3][3] == b[3][3];
    let affine = a[0][3] == 0.0 && a[1][3] == 0.0;
    (linear_same && affine).then(|| [b[3][0] - a[3][0], b[3][1] - a[3][1]])
}

/// Reports whether every transform node a chunk refers to moved by `by`.
pub(super) fn moves_with(
    derive: &Derive<'_>,
    world: &WorldChanges,
    key: ChunkKey,
    by: Option<[f32; 2]>,
) -> bool {
    let Some(record) = derive.scene.chunks.get(&key) else {
        return false;
    };
    record
        .derived
        .deps
        .iter()
        .filter(|object| object.kind == Kind::Transform)
        .all(|object| {
            world.moved.get(&object.slot).is_some_and(|moved| {
                let delta = translation_between(&moved.before, &moved.after);
                delta
                    .zip(by)
                    .is_some_and(|(delta, by)| same_translation(delta, by))
            })
        })
}

/// Reports whether two translations are equal up to rounding error.
pub(super) fn same_translation(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
}

/// Returns `by` in whole pixels, when it is whole.
pub(crate) fn whole_pixels(by: [f32; 2]) -> Option<[i32; 2]> {
    let [x, y] = by;
    let whole = (x - x.round()).abs() < 1e-3 && (y - y.round()).abs() < 1e-3;
    whole.then(|| [x.round() as i32, y.round() as i32])
}
