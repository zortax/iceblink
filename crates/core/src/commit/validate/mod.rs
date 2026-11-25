//! Judging the state a transaction leaves behind.

mod chunks;
mod in_use;
pub(crate) mod items;
mod tree;

use iceblink::FontRegistry;
use iceblink::{ChunkKey, CommitError, OrderEntry};
use smallvec::SmallVec;

use crate::commit::journal::Journal;
use crate::commit::precedence::Earliest;
use crate::output::Outputs;
use crate::scene::Scene;
use crate::scene::kind::ObjectRef;
use crate::work::Work;

/// The most emptied reference lists kept for later commits.
// A virtual list that moves its 200 visible chunks every frame takes two lists a chunk.
const SPARE_LISTS: usize = 1024;

/// The changed chunk keys of a commit, and emptied reference lists their nets reuse.
#[derive(Debug, Default)]
pub(crate) struct ChunkNets {
    pub(crate) nets: Vec<ChunkNet>,
    spare: Vec<Vec<ObjectRef>>,
}

impl ChunkNets {
    /// Returns an empty reference list, reusing the capacity of one kept earlier.
    fn list(&mut self) -> Vec<ObjectRef> {
        self.spare.pop().unwrap_or_default()
    }

    /// Removes every net and keeps their reference lists, emptied, for later commits.
    pub(crate) fn clear(&mut self) {
        for net in self.nets.drain(..) {
            for mut list in [net.before, net.after].into_iter().flatten() {
                if self.spare.len() < SPARE_LISTS {
                    list.clear();
                    self.spare.push(list);
                }
            }
        }
    }
}

/// A chunk key the commit changed, with the objects its chunk refers to before and after.
#[derive(Debug)]
pub(crate) struct ChunkNet {
    pub(crate) key: ChunkKey,
    /// The objects the chunk referred to before the commit, or `None` when the key held no chunk.
    pub(crate) before: Option<Vec<ObjectRef>>,
    /// The objects the chunk refers to after the commit, or `None` when the key holds no chunk.
    pub(crate) after: Option<Vec<ObjectRef>>,
    /// Whether the commit recorded the chunk the key holds after it.
    pub(crate) recorded: bool,
    /// The objects of `after` that the chunk names only through props the commit wrote, by the id
    /// of an object the commit created at the slot of a removed one.
    pub(crate) renamed: SmallVec<[ObjectRef; 2]>,
}

/// Checks every rule of the state a transaction left behind, and counts the checks it ran in
/// `work`.
///
/// Every changed chunk key is added to `nets`, which must be empty, with the objects its chunk
/// refers to before and after the commit.
///
/// # Errors
///
/// Returns the broken rule attributed to the earliest command, and of that command's broken rules
/// the first in rule order: a stale reference or an invalid item of a recorded chunk, a removed
/// object that is still referred to, an order entry in two places, or a cycle.
pub(crate) fn validate(
    scene: &Scene,
    outputs: &Outputs,
    journal: &Journal,
    entries: &[OrderEntry],
    fonts: &dyn FontRegistry,
    work: &mut Work,
    nets: &mut ChunkNets,
) -> Result<(), CommitError> {
    let mut earliest = Earliest::default();
    chunks::nets(scene, journal, fonts, nets, &mut earliest);
    in_use::check(scene, outputs, journal, &nets.nets, work, &mut earliest);
    tree::check_cycles(scene, journal, &mut earliest);
    tree::check_order(scene, outputs, journal, entries, &mut earliest);
    earliest.into_result()
}
