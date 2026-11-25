//! The vector plan of a frame redrawn whole.

use iceblink::{DeviceRect, VectorPlan};

use crate::plan::pass::cells::InkCells;
use crate::plan::pass::coalesce::{cut, pairwise_disjoint, relative};
use crate::plan::pass::region;

/// Writes `plan` into `out` with every pass that was cut to the damage given its whole region back.
///
/// `path_inks` holds the ink of every path by its index in `Frame::paths`, and `size` is the
/// output's size. The passes, their paths, their clips and their sequence are kept, so `out` is
/// the plan the same frame gets with full damage.
pub fn uncut(plan: &VectorPlan, path_inks: &[DeviceRect], size: [u32; 2], out: &mut VectorPlan) {
    out.clone_from(plan);
    let VectorPlan { items, passes } = out;

    let mut covered = Vec::new();
    let mut cells = InkCells::default();
    for pass in passes.iter_mut().filter(|pass| pass.clamped) {
        let planned = &mut items[pass.items.start as usize..pass.items.end as usize];
        let Some(bounds) = planned
            .iter()
            .map(|item| path_inks[item.path as usize])
            .reduce(DeviceRect::union)
        else {
            continue;
        };

        let whole = region::aligned(bounds, size);
        covered.clear();
        for item in planned {
            let ink = cut(path_inks[item.path as usize], whole);
            covered.push(ink);
            item.ink = relative(ink, whole);
        }

        pass.region = whole;
        pass.instanced = pairwise_disjoint(&covered, &mut cells);
        pass.clamped = false;
    }
}
