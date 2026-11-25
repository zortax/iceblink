//! Creating order nodes, splicing their sequences and setting output roots.

use core::ops::Range;

use iceblink::{
    ArenaRange, CommitError, EffectId, OrderEntry, OrderId, OrderNode, OutputId, RejectReason,
};

use crate::commit::apply::Apply;
use crate::commit::journal::{Change, Op, OrderChange, RootWrite, Splice, place, unplace};
use crate::scene::record::OrderRecord;

impl Apply<'_> {
    pub(crate) fn create_order(&mut self, id: OrderId, node: OrderNode) -> Result<(), CommitError> {
        self.check_creatable(&self.scene.orders, id.slot(), id.generation())?;
        self.check_order_node(&node)?;

        let record = OrderRecord::new(id, node);
        let vacancy = self.scene.orders.create(id.slot(), id.generation(), record);
        self.journal.order_node(
            Change {
                slot: id.slot(),
                command: self.command,
                op: Op::Created(vacancy),
            },
            node.effect.is_some(),
        );
        Ok(())
    }

    pub(crate) fn set_order_effect(
        &mut self,
        id: OrderId,
        effect: Option<EffectId>,
    ) -> Result<(), CommitError> {
        let node = OrderNode { effect };
        self.check_order_node(&node)?;
        let Some(record) = self.scene.orders.get_mut(id.slot(), id.generation()) else {
            return Err(self.reject(RejectReason::StaleId));
        };

        let before = core::mem::replace(&mut record.node, node);
        self.journal.order_node(
            Change {
                slot: id.slot(),
                command: self.command,
                op: Op::Wrote(before),
            },
            before.effect.is_some() || effect.is_some(),
        );
        self.counters.property_writes += 1;
        Ok(())
    }

    fn check_order_node(&self, node: &OrderNode) -> Result<(), CommitError> {
        let effect = node.effect.is_none_or(|id| self.live_effect(id));
        self.require(effect, RejectReason::StaleId)
    }

    pub(crate) fn splice(
        &mut self,
        node: OrderId,
        at: u32,
        remove: u32,
        insert: &[OrderEntry],
        entries: ArenaRange,
    ) -> Result<(), CommitError> {
        let Some(record) = self.scene.orders.get(node.slot(), node.generation()) else {
            return Err(self.reject(RejectReason::StaleId));
        };
        let len = record.entries.len() as u64;
        for entry in insert {
            if let OrderEntry::Node(child) = entry {
                self.require(self.live_order(*child), RejectReason::StaleId)?;
            }
        }
        self.require(
            u64::from(at) + u64::from(remove) <= len,
            RejectReason::SpliceOutOfRange,
        )?;

        self.splice_unchecked(node.slot(), at, remove, insert, entries);
        self.counters.keys_spliced += remove + insert.len() as u32;
        Ok(())
    }

    /// Splices a live node's sequence within its range and journals the splice.
    pub(crate) fn splice_unchecked(
        &mut self,
        node: u32,
        at: u32,
        remove: u32,
        insert: &[OrderEntry],
        entries: ArenaRange,
    ) {
        let Some(record) = self.scene.orders.at_mut(node) else {
            return;
        };
        let range = at as usize..(at + remove) as usize;
        let inked = record.ink.content().is_some();

        let journal = &mut *self.journal;
        let first = journal.removed_entries.len();
        journal
            .removed_entries
            .extend_from_slice(&record.entries[range.clone()]);
        replace(&mut record.entries, range, insert);

        for &entry in &journal.removed_entries[first..] {
            match entry {
                OrderEntry::Chunk(key) => journal.unplaced.push((key, node)),
                OrderEntry::Node(_) => unplace(self.scene, entry, node),
            }
        }
        for &entry in insert {
            if place(self.scene, entry, node) {
                journal.doubled.push(entry);
            }
        }

        journal.orders.push(OrderChange::Splice(Splice {
            node,
            command: self.command,
            at,
            removed: ArenaRange {
                start: first as u32,
                len: remove,
            },
            inked,
            inserted: insert.len() as u32,
            entries,
        }));
    }

    pub(crate) fn set_root(
        &mut self,
        output: OutputId,
        root: Option<OrderId>,
    ) -> Result<(), CommitError> {
        let live = root.is_none_or(|id| self.live_order(id));
        self.require(live, RejectReason::StaleId)?;
        self.require(
            self.outputs.contains_key(&output),
            RejectReason::UnknownOutput,
        )?;

        let client = self.scene.client;
        let Some(held) = self.outputs.get_mut(&output) else {
            return Err(self.reject(RejectReason::UnknownOutput));
        };

        let before = core::mem::replace(&mut held.root, root.map(|id| (client, id)));
        self.journal.roots.push(RootWrite {
            output,
            command: self.command,
            before,
        });
        Ok(())
    }
}

/// Replaces the entries of `range` with `insert`.
fn replace(entries: &mut Vec<OrderEntry>, range: Range<usize>, insert: &[OrderEntry]) {
    // Splices mostly fill a sequence, empty it or rewrite its end, which need no general splice.
    if range.len() == insert.len() {
        entries[range].copy_from_slice(insert);
    } else if range.end == entries.len() {
        entries.truncate(range.start);
        entries.extend_from_slice(insert);
    } else {
        entries.splice(range, insert.iter().copied());
    }
}
