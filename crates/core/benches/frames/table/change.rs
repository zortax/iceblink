//! The changes that hide rows and show new ones in their place.

use iceblink::{OrderEntry, Transaction};

use crate::table::{Change, HEAD, Table};

impl Table {
    /// Removes the rows the change hides, with one splice of the table's node per run of them.
    pub(super) fn hide(&mut self, tx: &mut Transaction) {
        let rows = core::mem::take(&mut self.rows);
        let mut at = HEAD;
        let mut run = 0;
        for (index, row) in rows.into_iter().enumerate() {
            if self.hides(index) {
                Self::drop_row(tx, &row);
                run += 1;
                continue;
            }
            if run > 0 {
                tx.splice(self.table, at, run, &[]);
                run = 0;
            }
            at += 1;
            self.rows.push(row);
        }
        if run > 0 {
            tx.splice(self.table, at, run, &[]);
        }
    }

    /// Shows new rows where the change hid rows, with one splice of the table's node per run.
    pub(super) fn show(&mut self, tx: &mut Transaction) {
        let mut kept = core::mem::take(&mut self.rows).into_iter();
        let mut run: Vec<OrderEntry> = Vec::new();
        for index in 0..self.count {
            if self.hides(index) {
                let row = self.make_row(tx);
                run.push(OrderEntry::Node(row.node()));
                self.rows.push(row);
                continue;
            }
            if !run.is_empty() {
                let at = HEAD + (self.rows.len() - run.len()) as u32;
                tx.splice(self.table, at, 0, &run);
                run.clear();
            }
            self.rows.extend(kept.next());
        }
        if !run.is_empty() {
            let at = HEAD + (self.rows.len() - run.len()) as u32;
            tx.splice(self.table, at, 0, &run);
        }
    }

    /// Reports whether the change hides the row at position `index` of the full table.
    fn hides(&self, index: usize) -> bool {
        match self.change {
            Change::Search => true,
            Change::Filter => index % 4 == 1,
        }
    }

    /// Brings the rows in the port to their positions.
    pub(super) fn draw_rows(&mut self, tx: &mut Transaction) {
        let mut rows = core::mem::take(&mut self.rows);
        for (at, row) in rows.iter_mut().enumerate().take(self.visible) {
            self.draw_row(tx, row, at);
        }
        self.rows = rows;
    }
}
