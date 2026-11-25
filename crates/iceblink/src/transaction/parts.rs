//! Taking a transaction apart, and building one from recycled buffers.

use crate::id::{AnyId, IdSpace};
use crate::node::OrderEntry;
use crate::transaction::{Command, RecordedChunk, Transaction};

/// The contents of a transaction, for a compositor to apply.
#[derive(Debug)]
pub struct TransactionParts {
    /// The id space the transaction minted from, whose client is the transaction's.
    pub ids: IdSpace,
    /// The commands, in the order they were issued.
    pub commands: Vec<Command>,
    /// The recorded chunks, indexed by `Command::Record`.
    pub chunks: Vec<RecordedChunk>,
    /// The entries of every splice, indexed by `Command::Splice`.
    pub entries: Vec<OrderEntry>,
    /// Every id the transaction minted, in mint order.
    pub minted: Vec<AnyId>,
    /// Empty recycled chunks the transaction did not record into, whose buffers a later
    /// transaction can reuse.
    pub spare: Vec<RecordedChunk>,
}

impl Transaction {
    /// Takes the transaction apart for a compositor to apply.
    ///
    /// The minted ids move into the parts, so they are not freed when the transaction is dropped.
    pub fn into_parts(mut self) -> TransactionParts {
        // `Transaction` frees `minted` on drop, so every field is taken out and `minted` is left
        // empty.
        TransactionParts {
            ids: self.ids.clone(),
            commands: core::mem::take(&mut self.commands),
            chunks: core::mem::take(&mut self.chunks),
            entries: core::mem::take(&mut self.entries),
            minted: core::mem::take(&mut self.minted),
            spare: core::mem::take(&mut self.spare),
        }
    }

    /// Creates an empty transaction for the client of `ids` that reuses the buffers of `parts`.
    ///
    /// Every chunk of `parts.chunks` is emptied and joins the chunks of `parts.spare`, which are
    /// empty already. `record` and `empty_chunk` reuse them before they allocate a new one.
    pub fn from_recycled(ids: IdSpace, parts: TransactionParts) -> Self {
        let TransactionParts {
            ids: _,
            mut commands,
            mut chunks,
            mut entries,
            mut minted,
            mut spare,
        } = parts;

        commands.clear();
        entries.clear();
        minted.clear();
        for chunk in &mut chunks {
            chunk.clear();
        }
        spare.append(&mut chunks);

        Self {
            ids,
            commands,
            chunks,
            spare,
            entries,
            minted,
        }
    }
}
