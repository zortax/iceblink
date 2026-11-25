//! Transactions: batches of changes a client builds and a compositor applies atomically.

mod animations;
mod chunks;
mod command;
mod damage;
mod nodes;
mod order;
mod parts;
mod recorded;
mod recorder;
mod resources;
mod slots;

use crate::id::{AnyId, ClientId, Id, IdKind, IdSpace};
use crate::node::OrderEntry;

pub use self::command::Command;
pub use self::parts::TransactionParts;
pub use self::recorded::RecordedChunk;
pub use self::recorder::ChunkRecorder;

/// A batch of changes to chunks, nodes, slots, the order tree and resources.
///
/// A client builds a transaction on any thread, and a compositor applies all of it or none of it
/// at commit. Commands take effect in the order they are issued, so a later command on the same
/// object overrides an earlier one. Anything a transaction does not mention is unchanged.
///
/// Creating an object mints its id from the client's [`IdSpace`] immediately, so later commands
/// of the same transaction can refer to it. Dropping a transaction without taking it apart frees
/// the ids it minted.
#[derive(Debug)]
pub struct Transaction {
    ids: IdSpace,
    commands: Vec<Command>,
    chunks: Vec<RecordedChunk>,
    spare: Vec<RecordedChunk>,
    entries: Vec<OrderEntry>,
    minted: Vec<AnyId>,
}

impl Transaction {
    /// Creates an empty transaction for the client of `ids`, without recycled buffers.
    pub fn new(ids: IdSpace) -> Self {
        Self {
            ids,
            commands: Vec::new(),
            chunks: Vec::new(),
            spare: Vec::new(),
            entries: Vec::new(),
            minted: Vec::new(),
        }
    }

    /// Returns the client the transaction belongs to.
    pub fn client(&self) -> ClientId {
        self.ids.client()
    }

    /// Reports whether the transaction holds no command.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Removes a node, a slot or a resource. Removing an animation cancels it.
    pub fn remove(&mut self, id: impl Into<AnyId>) {
        self.commands.push(Command::Remove(id.into()));
    }

    fn mint<T: IdKind>(&mut self) -> Id<T> {
        let id = self.ids.mint::<T>();
        self.minted.push(id.into());
        id
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        self.ids.free(&self.minted);
    }
}
