//! Creating, writing and removing slab objects, with a journal entry for each change.

use iceblink::{CommitError, RejectReason};

use crate::commit::apply::Apply;
use crate::commit::journal::{Change, Op};
use crate::scene::slab::Slab;
use crate::scene::stored::Stored;

impl Apply<'_> {
    /// Checks that a create may use `slot` with `generation`.
    pub(crate) fn check_creatable<R>(
        &self,
        slab: &Slab<R>,
        slot: u32,
        generation: u32,
    ) -> Result<(), CommitError> {
        self.require(slab.can_create(slot, generation), RejectReason::StaleId)
    }
}

/// Stores a created object and journals the create.
pub(crate) fn create<R: Stored>(
    command: u32,
    slab: &mut Slab<R>,
    changes: &mut Vec<Change<R>>,
    slot: u32,
    generation: u32,
    record: R,
) {
    let vacancy = slab.create(slot, generation, record);
    changes.push(Change {
        slot,
        command,
        op: Op::Created(vacancy),
    });
}

/// Writes the value of a live object and journals the old value, or reports that the id is not
/// live.
pub(crate) fn write<R: Stored>(
    command: u32,
    slab: &mut Slab<R>,
    changes: &mut Vec<Change<R>>,
    slot: u32,
    generation: u32,
    value: R::Value,
) -> bool {
    let Some(record) = slab.get_mut(slot, generation) else {
        return false;
    };

    let before = core::mem::replace(record.value_mut(), value);
    changes.push(Change {
        slot,
        command,
        op: Op::Wrote(before),
    });
    true
}

/// Takes a live object out of its slab and journals the removal, or reports that it is not live.
pub(crate) fn take<R: Stored>(
    command: u32,
    slab: &mut Slab<R>,
    changes: &mut Vec<Change<R>>,
    slot: u32,
    generation: u32,
) -> bool {
    if slab.get(slot, generation).is_none() {
        return false;
    }

    let Some(record) = slab.remove(slot) else {
        return false;
    };
    changes.push(Change {
        slot,
        command,
        op: Op::Removed(record),
    });
    true
}
