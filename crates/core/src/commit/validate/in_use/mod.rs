//! Checking that nothing refers to a removed object after the commit.

mod changed;
mod placed;

use rustc_hash::FxHashMap;

use crate::commit::journal::{Change, Journal, Op};
use crate::commit::precedence::Earliest;
use crate::commit::validate::ChunkNet;
use crate::output::Outputs;
use crate::scene::Scene;
use crate::scene::kind::ObjectRef;
use crate::scene::stored::Stored;
use crate::work::Work;

use self::changed::Changed;
use self::placed::placed_nodes;

/// How many references each object gains or loses by the commit.
type Delta = FxHashMap<ObjectRef, i64>;

/// Notes in `earliest` every removal of an object that something other than a chunk the commit
/// recorded still refers to, once the commit is applied, and counts the commits that removed one in
/// `work`.
///
/// A node, an order entry or the props of a chunk that names an object the commit created at the
/// slot of a removed one keeps only the new object in use. The other references of a chunk the
/// commit did not record refer to the objects their slots held before the commit.
pub(super) fn check(
    scene: &Scene,
    outputs: &Outputs,
    journal: &Journal,
    chunks: &[ChunkNet],
    work: &mut Work,
    earliest: &mut Earliest,
) {
    let removed = removes(journal);
    if !removed.any() {
        return;
    }

    work.use_checks += 1;

    // A removed order node is checked by its placements alone, so the reference counts are only
    // needed when the commit removed an object of another kind.
    if !removed.named {
        placed_nodes(scene, outputs, journal, earliest);
        return;
    }

    let mut delta = Delta::default();
    for net in chunks {
        for &object in net.before.iter().flatten() {
            *delta.entry(object).or_default() -= 1;
        }
        // A recorded chunk that refers to a removed object breaks the chunk's own rules.
        if net.recorded {
            continue;
        }
        let kept = net.after.iter().flatten();
        for &object in kept.filter(|object| !net.renamed.contains(object)) {
            *delta.entry(object).or_default() += 1;
        }
    }

    let changed = Changed::of(scene, journal);
    changed.count(&mut delta);
    changed.forget_replacements(&mut delta);
    changed.named(&delta, earliest);

    placed_nodes(scene, outputs, journal, earliest);
}

/// The kinds of object a commit removed.
struct Removed {
    /// An order node.
    orders: bool,
    /// An object other values or chunks can refer to.
    named: bool,
}

impl Removed {
    fn any(&self) -> bool {
        self.orders || self.named
    }
}

/// Returns the kinds of object the commit removed.
fn removes(journal: &Journal) -> Removed {
    fn any<R: Stored>(changes: &[Change<R>]) -> bool {
        changes
            .iter()
            .any(|change| matches!(change.op, Op::Removed(_)))
    }

    let orders = !journal.removed_orders.is_empty();
    let named = any(&journal.transforms)
        || any(&journal.scrolls)
        || any(&journal.clips)
        || any(&journal.effects)
        || any(&journal.paints)
        || any(&journal.params)
        || any(&journal.images)
        || any(&journal.textures)
        || any(&journal.shaders);
    Removed { orders, named }
}
