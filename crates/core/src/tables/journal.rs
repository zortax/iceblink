//! The change journal of a frame table, which several readers catch up on independently.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

/// How many changed slots a reader may fall behind before it is sent the whole table.
const CHANGE_HISTORY: usize = 4096;

/// A table and its contents at one moment, as a reader last saw them.
///
/// A version of another table, or the default version, asks for the whole table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TableVersion {
    instance: u64,
    revision: u64,
}

/// Whether a journal could list every slot changed since a version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Coverage {
    /// Every changed slot was listed.
    Delta,
    /// The version belongs to another table or fell behind the journal.
    All,
}

// Tells apart tables whose revision counters hold the same number.
static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// The slots of one table changed recently, in the order they changed.
#[derive(Debug)]
pub(crate) struct Journal {
    instance: u64,
    revision: u64,
    /// The changed slots, the newest at `revision` and each older one a revision before.
    changes: VecDeque<u32>,
}

impl Default for Journal {
    fn default() -> Self {
        Self {
            instance: NEXT_INSTANCE.fetch_add(1, Ordering::Relaxed),
            revision: 0,
            changes: VecDeque::new(),
        }
    }
}

impl Journal {
    /// Returns the version of the table as it is now.
    pub(crate) fn version(&self) -> TableVersion {
        TableVersion {
            instance: self.instance,
            revision: self.revision,
        }
    }

    /// Records that `slot` changed.
    pub(crate) fn note(&mut self, slot: u32) {
        self.revision += 1;

        if self.changes.len() == CHANGE_HISTORY {
            self.changes.pop_front();
        }
        self.changes.push_back(slot);
    }

    /// Calls `changed` with every slot changed since `version`, possibly more than once per slot.
    ///
    /// Returns `Coverage::All` without calling it when the journal cannot list them, and the
    /// reader then needs the whole table.
    pub(crate) fn changes_since(
        &self,
        version: TableVersion,
        mut changed: impl FnMut(u32),
    ) -> Coverage {
        if version.instance != self.instance || version.revision > self.revision {
            return Coverage::All;
        }
        if version.revision == self.revision {
            return Coverage::Delta;
        }

        // Every note is one revision, so the slot changed at `revision` lies at a known index.
        let held = self.changes.len() as u64;
        let behind = self.revision - version.revision;
        if behind > held {
            return Coverage::All;
        }

        for &slot in self.changes.range((held - behind) as usize..) {
            changed(slot);
        }
        Coverage::Delta
    }
}
