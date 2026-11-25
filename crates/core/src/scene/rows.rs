//! The extents of an order node's entries, for finding those that meet a band.

use core::ops::Range;

use iceblink::DeviceRect;
use iceblink::geom::{Point, Size};

/// The extents of an order node's entries with an ink, in content space for a carried node, when
/// each entry starts and ends no higher than the one before it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Rows {
    rows: Vec<Row>,
}

/// One entry's extent.
///
/// Rows compare by their vertical extents and entries. The horizontal extent serves the union of
/// a carried node's rows, and a patch of an on-device node can leave it wider than the entry.
#[derive(Clone, Copy, Debug)]
struct Row {
    top: i32,
    bottom: i32,
    left: i32,
    right: i32,
    /// The entry's position in the node's sequence.
    entry: u32,
}

impl PartialEq for Row {
    fn eq(&self, other: &Self) -> bool {
        (self.top, self.bottom, self.entry) == (other.top, other.bottom, other.entry)
    }
}

impl Eq for Row {}

impl Row {
    fn new(entry: usize, ink: DeviceRect) -> Self {
        Self {
            top: ink.origin.y,
            bottom: ink.origin.y.saturating_add(ink.size.height),
            left: ink.origin.x,
            right: ink.origin.x.saturating_add(ink.size.width),
            entry: entry as u32,
        }
    }

    fn rect(&self) -> DeviceRect {
        DeviceRect::new(
            Point::new(self.left, self.top),
            Size::new(self.right - self.left, self.bottom - self.top),
        )
    }

    /// Reports whether the row starts and ends no higher than `previous`.
    fn follows(&self, previous: &Row) -> bool {
        previous.top <= self.top && previous.bottom <= self.bottom
    }
}

impl Rows {
    /// Removes every row, keeping the capacity.
    pub(crate) fn clear(&mut self) {
        self.rows.clear();
    }

    /// Adds the entry at position `entry` with content `ink`, and reports whether the entries
    /// are still ordered.
    pub(crate) fn push(&mut self, entry: usize, ink: DeviceRect) -> bool {
        let row = Row::new(entry, ink);
        let ordered = self.rows.last().is_none_or(|last| row.follows(last));
        self.rows.push(row);
        ordered
    }

    /// Gives the row of the entry at position `entry` the extent of `ink`, and reports whether it
    /// could: the entry has a row and the rows stay ordered.
    ///
    /// Nothing changes when it could not.
    pub(crate) fn update(&mut self, entry: usize, ink: DeviceRect) -> bool {
        let Ok(at) = self
            .rows
            .binary_search_by_key(&(entry as u32), |row| row.entry)
        else {
            return false;
        };

        let row = Row::new(entry, ink);
        let after_previous = at == 0 || row.follows(&self.rows[at - 1]);
        let before_next = self.rows.get(at + 1).is_none_or(|next| next.follows(&row));
        if !(after_previous && before_next) {
            return false;
        }

        self.rows[at] = row;
        true
    }

    /// Reports whether each of `other`'s rows lies vertically inside the row of the same entry.
    pub(crate) fn holds(&self, other: &Rows) -> bool {
        other.rows.iter().all(|row| {
            self.rows
                .binary_search_by_key(&row.entry, |held| held.entry)
                .is_ok_and(|at| self.rows[at].top <= row.top && row.bottom <= self.rows[at].bottom)
        })
    }

    /// Returns the range of rows whose extent meets the band from `top` to `bottom`, in content
    /// space.
    pub(crate) fn meeting(&self, top: i32, bottom: i32) -> Range<usize> {
        let start = self.rows.partition_point(|row| row.bottom <= top);
        let end = self.rows.partition_point(|row| row.top < bottom);
        start..end.max(start)
    }

    /// Returns the position in the node's sequence of the entry of row `row`.
    pub(crate) fn entry(&self, row: usize) -> Option<usize> {
        self.rows.get(row).map(|row| row.entry as usize)
    }

    /// Follows a splice of the node's sequence that replaced `removed` entries at position `at`
    /// with `inserted` others: the rows of the removed entries are dropped and the later ones
    /// renumbered.
    pub(crate) fn splice(&mut self, at: usize, removed: usize, inserted: usize) {
        let (at, end) = (at as u32, (at + removed) as u32);
        self.rows.retain(|row| row.entry < at || row.entry >= end);

        for row in &mut self.rows {
            if row.entry >= end {
                row.entry = row.entry - removed as u32 + inserted as u32;
            }
        }
    }

    /// Gives the entry at position `entry` a row with content `ink`, or no row for `None`, in
    /// sequence order.
    pub(crate) fn set(&mut self, entry: usize, ink: Option<DeviceRect>) {
        let at = self
            .rows
            .partition_point(|row| (row.entry as usize) < entry);
        let held = self
            .rows
            .get(at)
            .is_some_and(|row| row.entry as usize == entry);

        match (held, ink) {
            (true, Some(ink)) => self.rows[at] = Row::new(entry, ink),
            (true, None) => {
                self.rows.remove(at);
            }
            (false, Some(ink)) => self.rows.insert(at, Row::new(entry, ink)),
            (false, None) => {}
        }
    }

    /// Reports whether each row starts and ends no higher than the one before it.
    pub(crate) fn ordered(&self) -> bool {
        self.rows.windows(2).all(|pair| pair[1].follows(&pair[0]))
    }

    /// Returns the union of the rows' extents, as `DeviceRect::union` folds them in sequence order.
    pub(crate) fn union(&self) -> Option<DeviceRect> {
        let mut edges: Option<[i32; 4]> = None;
        for row in &self.rows {
            if row.left >= row.right || row.top >= row.bottom {
                continue;
            }
            edges = Some(
                edges.map_or([row.left, row.top, row.right, row.bottom], |held| {
                    [
                        held[0].min(row.left),
                        held[1].min(row.top),
                        held[2].max(row.right),
                        held[3].max(row.bottom),
                    ]
                }),
            );
        }

        // A union of empty rows only is the last of them.
        let Some([left, top, right, bottom]) = edges else {
            return self.rows.last().map(Row::rect);
        };
        Some(DeviceRect::new(
            Point::new(left, top),
            Size::new(right - left, bottom - top),
        ))
    }
}
