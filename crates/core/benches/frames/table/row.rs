//! One row of the table: eleven order nodes in four levels, and the chunks of its cells.

use iceblink::{ChunkKey, OrderEntry, OrderId, OrderNode, PaintKind, Transaction};

use crate::items::{props, quad, run};
use crate::table::{ROW, Table};

/// The order nodes of a row below the row's own node, as the parent and the chunks each lists.
///
/// A parent of `None` places the node in the row's node, and `Some(n)` in the node made `n`th.
const CELLS: [(Option<usize>, u32); 10] = [
    (None, 3),
    (None, 1),
    (Some(1), 3),
    (Some(1), 1),
    (Some(3), 3),
    (None, 1),
    (Some(5), 3),
    (None, 1),
    (Some(7), 2),
    (None, 3),
];

/// The chunks a row's nodes list, its own node's included.
const CHUNKS: u32 = 22;

/// Returns the parts of a row's chunks that draw: the row's background and every third label.
///
/// The others are boxes without paint, whose keys their nodes list all the same.
fn painted() -> impl Iterator<Item = u32> {
    (0..CHUNKS).filter(|part| part % 3 == 2 || *part == 0)
}

/// One row of the table.
pub(crate) struct Row {
    /// The owner of the row's chunk keys.
    pub(crate) id: u64,
    /// The row's order nodes with how many entries each lists, the row's own node first.
    pub(crate) nodes: Vec<(OrderId, u32)>,
    /// The position the row's chunks were recorded at, if they were.
    pub(crate) drawn: Option<usize>,
}

impl Row {
    /// Returns the row's own order node.
    pub(crate) fn node(&self) -> OrderId {
        self.nodes[0].0
    }
}

impl Table {
    /// Creates the order nodes of a new row, listing chunks it has not recorded.
    pub(crate) fn make_row(&mut self, tx: &mut Transaction) -> Row {
        let id = self.next;
        self.next += 1;
        let mut keys = (0..CHUNKS).map(|part| OrderEntry::Chunk(ChunkKey::new(id, part)));

        let mut lists: Vec<Vec<OrderEntry>> = vec![keys.by_ref().take(1).collect()];
        let mut nodes = vec![tx.create_order(OrderNode::default())];
        for (parent, chunks) in CELLS {
            let node = tx.create_order(OrderNode::default());
            lists[parent.map_or(0, |index| index + 1)].push(OrderEntry::Node(node));
            lists.push(keys.by_ref().take(chunks as usize).collect());
            nodes.push(node);
        }
        // Children are filled before their parents.
        for (node, entries) in nodes.iter().zip(&lists).rev() {
            tx.splice(*node, 0, 0, entries);
        }
        Row {
            id,
            nodes: nodes
                .into_iter()
                .zip(lists.iter().map(|list| list.len() as u32))
                .collect(),
            drawn: None,
        }
    }

    /// Empties and removes the nodes of `row`, and removes the chunks it recorded.
    pub(crate) fn drop_row(tx: &mut Transaction, row: &Row) {
        for &(node, listed) in row.nodes.iter().rev() {
            tx.splice(node, 0, listed, &[]);
        }
        for &(node, _) in &row.nodes {
            tx.remove(node);
        }

        if row.drawn.is_some() {
            for part in painted() {
                tx.remove_chunk(ChunkKey::new(row.id, part));
            }
        }
    }

    /// Brings the chunks of `row` to position `at`: records them if the row has none, and
    /// offsets them from the position they were recorded at otherwise.
    pub(crate) fn draw_row(&self, tx: &mut Transaction, row: &mut Row, at: usize) {
        match row.drawn {
            None => {
                for part in painted() {
                    self.record_cell(tx, ChunkKey::new(row.id, part), at);
                }
                row.drawn = Some(at);
            }
            Some(drawn) if drawn != at => {
                let offset = [0.0, (at as f32 - drawn as f32) * ROW];
                for part in painted() {
                    tx.set_offset(ChunkKey::new(row.id, part), offset);
                }
            }
            Some(_) => {}
        }
    }

    /// Records the cell of `key` in the row at position `at`: a background quad for the first
    /// part, a label for the others.
    fn record_cell(&self, tx: &mut Transaction, key: ChunkKey, at: usize) {
        let y = at as f32 * ROW;
        let x = 10.0 + key.part as f32 * 45.0;
        let mut chunk = tx.record(key, props(self.content, Some(self.port)));
        if key.part == 0 {
            let fill = chunk.paint_ref(PaintKind::SOLID, self.zebra);
            chunk.quad(quad([x, y, 44.0, ROW], fill));
        } else {
            let fill = chunk.paint_ref(PaintKind::SOLID, self.black);
            let text = format!("#{} {}", key.owner, key.part);
            let glyphs = chunk.glyphs(&self.layout.line(&text, 12.0, [x, y + 20.0]));
            chunk.glyph_run(run(self.font, glyphs, fill));
        }
    }
}
