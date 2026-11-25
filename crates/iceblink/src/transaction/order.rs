//! Transaction commands on the order tree.

use crate::id::{EffectId, OrderId, OutputId};
use crate::item::ArenaRange;
use crate::node::{OrderEntry, OrderNode};
use crate::transaction::{Command, Transaction};

impl Transaction {
    /// Creates an order node with an empty sequence.
    pub fn create_order(&mut self, node: OrderNode) -> OrderId {
        let id = self.mint();
        self.commands.push(Command::CreateOrder(id, node));
        id
    }

    /// Writes the effect of an order node.
    pub fn set_order_effect(&mut self, id: OrderId, effect: Option<EffectId>) {
        self.commands.push(Command::SetOrderEffect(id, effect));
    }

    /// Replaces `remove` entries of an order node's sequence, from position `at`, with `insert`.
    pub fn splice(&mut self, node: OrderId, at: u32, remove: u32, insert: &[OrderEntry]) {
        let start = self.entries.len() as u32;
        self.entries.extend_from_slice(insert);
        self.commands.push(Command::Splice {
            node,
            at,
            remove,
            insert: ArenaRange {
                start,
                len: insert.len() as u32,
            },
        });
    }

    /// Sets the order node an output draws, or `None` to draw nothing.
    pub fn set_root(&mut self, output: OutputId, root: Option<OrderId>) {
        self.commands.push(Command::SetRoot(output, root));
    }
}
