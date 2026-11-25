//! The clients of a compositor and the buffers their transactions reuse.

use iceblink::{
    AnyId, ClientId, Command, IdSpace, OrderEntry, RecordedChunk, Transaction, TransactionParts,
};

use crate::scene::Scene;

/// The most recycled chunks a client keeps for later transactions.
const SPARE_LIMIT: usize = 4096;

/// A registered client, with its ids, its scene and the buffers its transactions reuse.
#[derive(Debug)]
pub(crate) struct Client {
    pub(crate) ids: IdSpace,
    pub(crate) scene: Scene,
    pub(crate) recycled: Recycled,
}

/// Buffers of committed transactions, handed to the client's next transaction.
#[derive(Debug, Default)]
pub(crate) struct Recycled {
    commands: Vec<Command>,
    chunks: Vec<RecordedChunk>,
    entries: Vec<OrderEntry>,
    minted: Vec<AnyId>,
    spare: Vec<RecordedChunk>,
    /// The emptied recordings of the commit being applied, which join `spare` once it is kept.
    pub(crate) returned: Vec<RecordedChunk>,
}

impl Client {
    pub(crate) fn new(client: ClientId) -> Self {
        Self {
            ids: IdSpace::new(client),
            scene: Scene::new(client),
            recycled: Recycled::default(),
        }
    }

    /// Returns a transaction that reuses the buffers of earlier commits.
    pub(crate) fn transaction(&mut self) -> Transaction {
        let recycled = &mut self.recycled;
        let parts = TransactionParts {
            ids: self.ids.clone(),
            commands: core::mem::take(&mut recycled.commands),
            chunks: core::mem::take(&mut recycled.chunks),
            entries: core::mem::take(&mut recycled.entries),
            minted: core::mem::take(&mut recycled.minted),
            spare: core::mem::take(&mut recycled.spare),
        };
        Transaction::from_recycled(self.ids.clone(), parts)
    }

    /// Keeps the buffers of a committed transaction for the next one.
    pub(crate) fn keep(&mut self, mut parts: TransactionParts) {
        parts.commands.clear();
        parts.chunks.clear();
        parts.entries.clear();
        parts.minted.clear();

        let recycled = &mut self.recycled;
        recycled.commands = parts.commands;
        recycled.chunks = parts.chunks;
        recycled.entries = parts.entries;
        recycled.minted = parts.minted;

        let mut spare = parts.spare;
        spare.append(&mut recycled.spare);
        spare.append(&mut recycled.returned);
        spare.truncate(SPARE_LIMIT);
        recycled.spare = spare;
    }
}
