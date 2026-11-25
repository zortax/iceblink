//! The keys of vector passes, and the digests verification checks them against.

mod digests;
mod fold;

use std::hash::Hasher;

use iceblink::{FRAME_NO_CLIP, FramePaint, FramePath, PassKey, VectorPlan};

pub(crate) use self::digests::PassDigests;

use self::fold::{Outlines, fold_pass};
use crate::tables::table::FrameTable;

/// Gives every pass of `plan` that can be keyed its key.
///
/// A pass is keyed when it is not clamped and none of its paths has a residual clip. The key
/// folds outlines by the address of their `Arc`s. Other passes get `PassKey::NONE`.
pub(crate) fn assign(plan: &mut VectorPlan, paths: &[FramePath], paints: &FrameTable<FramePaint>) {
    let VectorPlan { items, passes } = plan;
    for pass in passes.iter_mut() {
        let planned = &items[pass.items.start as usize..pass.items.end as usize];
        let keyed = !pass.clamped && planned.iter().all(|item| item.residual == FRAME_NO_CLIP);
        pass.key = if keyed {
            key(fold_pass(pass, planned, paths, paints, Outlines::Addresses))
        } else {
            PassKey::NONE
        };
    }
}

/// Returns the key of a folded pass, which is never `PassKey::NONE`.
fn key(hasher: impl Hasher) -> PassKey {
    PassKey(hasher.finish().max(1))
}
