//! The pending scrolls of the outputs, which collect the damage of scroll offset changes.

use iceblink::OutputId;
use iceblink::damage::DamageSet;
use rustc_hash::FxHashSet;
use smallvec::SmallVec;

use crate::derive::Derive;
use crate::derive::scroll::Moves;
use crate::geometry::coverage::Partial;
use crate::output::scroll::{Late, Moving, PendingScroll};

/// Returns the pending scroll of scroll node `scroll` on output `output`, created when there is
/// none, with the commit's move `by` added once.
pub(super) fn pending_scroll<'a>(
    derive: &'a mut Derive<'_>,
    moves: &mut Moves,
    scroll: u32,
    output: OutputId,
    by: Option<[f32; 2]>,
) -> Option<&'a mut PendingScroll> {
    let client = derive.scene.client;
    let output_id = output;
    let first = moves.added.add((output, scroll));
    let output = derive.outputs.get_mut(&output)?;
    let bound = output.bounds();
    let index = match output
        .scrolls
        .iter()
        .position(|pending| pending.client == client && pending.scroll == scroll)
    {
        Some(index) => {
            let pending = &mut output.scrolls[index];
            if first {
                if let Some(before) = pending.by {
                    moves.again.push((output_id, scroll, before));
                }
                pending.by = match (pending.by, by) {
                    (Some(held), Some(more)) => Some([held[0] + more[0], held[1] + more[1]]),
                    _ => None,
                };
            }
            index
        }
        None => {
            let moving = Moving::new(bound);
            let mut still = DamageSet::new();
            still.set_bound(Some(bound));
            output.scrolls.push(PendingScroll {
                client,
                scroll,
                by,
                moving,
                still,
                contained: true,
                partial: Partial::NONE,
                spreads: false,
                entries: FxHashSet::default(),
                late: SmallVec::new(),
            });
            output.scrolls.len() - 1
        }
    };
    output.scrolls.get_mut(index)
}

/// Keeps each output's damage with the pending scrolls this commit moved again, once the commit's
/// damage is derived.
pub(crate) fn keep_late(derive: &mut Derive<'_>, moves: &Moves) {
    let client = derive.scene.client;
    for &(output, scroll, before) in &moves.again {
        let Some(output) = derive.outputs.get_mut(&output) else {
            continue;
        };
        let damage = output.pending;
        if let Some(pending) = output
            .scrolls
            .iter_mut()
            .find(|pending| pending.client == client && pending.scroll == scroll)
        {
            pending.late.push(Late { damage, before });
        }
    }
}
