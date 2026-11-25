//! Collecting the damage of scroll offset changes, which the next frame may answer with a shift.

mod accumulate;
mod admit;
mod carried;
mod defer;
mod pending;
mod translation;
mod travel;

pub(crate) use self::accumulate::{EntryMove, accumulate};
pub(crate) use self::carried::carried;
pub(crate) use self::pending::keep_late;
pub(crate) use self::translation::whole_pixels;
pub(crate) use self::travel::Travels;

use self::defer::{CarriedMove, defer};
use self::pending::pending_scroll;
use self::translation::{moves_with, same_translation, translation_between};

use iceblink::OutputId;
use smallvec::SmallVec;

use crate::derive::Derive;
use crate::derive::world::WorldChanges;
use crate::small::{SmallMap, SmallSet};

/// The device translations of the content of scroll nodes in one commit, and the pending scrolls
/// they were added to.
#[derive(Debug, Default)]
pub(crate) struct Moves {
    /// How far the content that applies each scroll node moved, by slot.
    by: SmallMap<u32, Option<[f32; 2]>, 4>,
    /// The outputs and scroll nodes whose pending scroll already took this commit's move.
    added: SmallSet<(OutputId, u32), 4>,
    /// The outputs and scroll nodes whose pending scroll an earlier commit moved, with how far.
    again: SmallVec<[(OutputId, u32, [f32; 2]); 1]>,
}

impl Moves {
    /// Returns how far the content that applies scroll node `scroll` moved on the device, or
    /// `None` when it did not move by one translation.
    fn of(&mut self, derive: &Derive<'_>, world: &WorldChanges, scroll: u32) -> Option<[f32; 2]> {
        *self.by.get_or_insert_with(scroll, || {
            let record = derive.scene.scrolls.at(scroll)?;
            let mut by: Option<[f32; 2]> = None;
            for &node in &record.dependents {
                let applies = derive
                    .scene
                    .transforms
                    .at(node)
                    .is_some_and(|t| t.node.scroll.is_some_and(|id| id.slot() == scroll));
                if !applies {
                    continue;
                }
                let moved = world.moved.get(&node)?;
                let delta = translation_between(&moved.before, &moved.after)?;
                match by {
                    Some(held) if !same_translation(held, delta) => return None,
                    _ => by = Some(delta),
                }
            }
            by
        })
    }
}
