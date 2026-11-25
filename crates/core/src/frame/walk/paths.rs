//! Where the frame paths of a lowering are drawn now.

use iceblink::Matrix4;
use smallvec::SmallVec;

use crate::frame::lower::View;
use crate::scene::chunk::Lowered;

/// Returns the device translation of the frame paths placed under each client space of
/// `lowered`, from where they were lowered to where the spaces and the chunk offset `offset`
/// put them now.
pub(super) fn path_moves(
    view: View<'_>,
    lowered: &Lowered,
    offset: [f32; 2],
) -> SmallVec<[(u32, [f64; 2]); 1]> {
    let point = |world: &Matrix4, at: [f32; 2]| world.transform_vector4([at[0], at[1], 0.0, 1.0]);
    let placements = lowered.content.iter().flat_map(|stamp| &stamp.worlds);
    placements
        .filter_map(|placement| {
            let now = view.transforms.at(placement.slot)?.world;
            let after = point(&now, offset);
            let before = point(&placement.world, lowered.base);
            let by = [
                f64::from(after[0] - before[0]),
                f64::from(after[1] - before[1]),
            ];
            Some((placement.slot, by))
        })
        .collect()
}
