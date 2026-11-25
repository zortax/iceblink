//! The travel of scroll nodes whose carried content a commit moved by whole pixels.

use smallvec::SmallVec;

use crate::derive::links::Cause;
use crate::derive::scroll::translation_between;
use crate::derive::world::WorldChanges;
use crate::scene::Scene;

/// The scroll nodes whose travel a commit changed, with the travel each had before.
#[derive(Debug, Default)]
pub(crate) struct Travels {
    before: SmallVec<[(u32, [i32; 2]); 1]>,
}

impl Travels {
    /// Adds a commit's whole-pixel scrolls to the travel of their scroll nodes, and returns the
    /// travels they replaced.
    ///
    /// `dirty` holds the transform nodes the commit made stale, with why. A scroll node's offset
    /// change is a whole-pixel scroll when every transform node that applies the offset moved by
    /// the same translation of whole pixels.
    pub(crate) fn advance(scene: &mut Scene, dirty: &[(u32, Cause)], world: &WorldChanges) -> Self {
        let mut scrolls: SmallVec<[u32; 1]> = dirty
            .iter()
            .filter_map(|&(_, cause)| match cause {
                Cause::Scroll(scroll) => Some(scroll),
                Cause::Write => None,
            })
            .collect();
        scrolls.sort_unstable();
        scrolls.dedup();

        let mut travels = Self::default();
        for scroll in scrolls {
            let Some(by) = whole_move(scene, world, scroll) else {
                continue;
            };
            let Some(record) = scene.scrolls.at_mut(scroll) else {
                continue;
            };
            travels.before.push((scroll, record.travel));
            record.travel = [
                record.travel[0].saturating_add(by[0]),
                record.travel[1].saturating_add(by[1]),
            ];
        }
        travels
    }

    /// Reports whether the commit moved the content scroll node `scroll` carries by its travel.
    pub(crate) fn moved(&self, scroll: u32) -> bool {
        self.before.iter().any(|&(held, _)| held == scroll)
    }

    /// Returns the scroll nodes whose content the commit moved by their travel.
    pub(crate) fn scrolls(&self) -> impl Iterator<Item = u32> + '_ {
        self.before.iter().map(|&(scroll, _)| scroll)
    }

    /// Returns the travel scroll node `scroll` had before the commit.
    pub(crate) fn before(&self, scene: &Scene, scroll: u32) -> [i32; 2] {
        self.before
            .iter()
            .find(|&&(held, _)| held == scroll)
            .map_or_else(|| scene.travel(scroll), |&(_, travel)| travel)
    }
}

/// Returns the translation of whole pixels every transform node applying scroll node `scroll`
/// moved by, when they all moved by the same one.
fn whole_move(scene: &Scene, world: &WorldChanges, scroll: u32) -> Option<[i32; 2]> {
    let record = scene.scrolls.at(scroll)?;
    let mut by = None;
    for &node in &record.dependents {
        let applies = scene
            .transforms
            .at(node)
            .is_some_and(|t| t.node.scroll.is_some_and(|id| id.slot() == scroll));
        if !applies {
            continue;
        }
        let delta = match world.moved.get(&node) {
            Some(moved) => translation_between(&moved.before, &moved.after)?,
            None => [0.0; 2],
        };
        // Inks move by the travel exactly, so the move is taken only when it is exact.
        if delta.iter().any(|axis| *axis != axis.round()) {
            return None;
        }
        let delta = delta.map(|axis| axis as i32);
        if by.is_some_and(|held| held != delta) {
            return None;
        }
        by = Some(delta);
    }
    by
}
