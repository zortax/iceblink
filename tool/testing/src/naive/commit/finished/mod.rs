//! Checks of the state a whole transaction leaves.

mod in_use;
mod references;
mod trees;

use iceblink::CommitError;

use crate::naive::commit::check::items;
use crate::naive::commit::draft::Draft;
use crate::naive::commit::precedence::Earliest;

/// Checks the finished draft and fails with the broken rule attributed to the earliest command.
///
/// The rules are that every chunk the transaction recorded is well formed, that nothing still
/// refers to an object it removed, that no order node or chunk key appears twice, and that
/// transform parents, clip parents and the order tree form trees.
pub(crate) fn check(draft: &Draft<'_>) -> Result<(), CommitError> {
    let mut earliest = Earliest::default();
    for (key, index) in &draft.recorded {
        let chunk = &draft.client.chunks[key];
        if let Err(reason) = items::recorded(&draft.client, draft.fonts, &chunk.recorded) {
            earliest.note(*index, reason);
        }
    }

    in_use::check(draft, &mut earliest);
    trees::check(draft, &mut earliest);

    earliest.into_result()
}
