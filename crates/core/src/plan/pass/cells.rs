//! A uniform grid over a pass's inks, for asking which of them a rectangle meets.

use iceblink::DeviceRect;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

/// The edge of one cell, in device pixels.
const CELL: i32 = 64;

// Ink far outside the output lands in the outermost cells. The mapping stays monotonic, so two
// rectangles that meet still share a cell.
const LOWEST: i32 = -64;
const HIGHEST: i32 = 256;

/// The inks of one pass, by the cells they cover.
#[derive(Debug, Default)]
pub(crate) struct InkCells {
    cells: FxHashMap<(i32, i32), SmallVec<[u32; 4]>>,
}

impl InkCells {
    pub(crate) fn clear(&mut self) {
        self.cells.clear();
    }

    /// Records that ink `index` covers `ink`.
    pub(crate) fn insert(&mut self, index: usize, ink: DeviceRect) {
        let (columns, rows) = span(ink);
        for column in columns {
            for row in rows.clone() {
                self.cells
                    .entry((column, row))
                    .or_default()
                    .push(index as u32);
            }
        }
    }

    /// Reports whether `rect` meets one of `inks` whose index `admits` accepts.
    ///
    /// `inks` is the list the indices were recorded from.
    pub(crate) fn meets(
        &self,
        rect: DeviceRect,
        inks: &[DeviceRect],
        mut admits: impl FnMut(usize) -> bool,
    ) -> bool {
        if self.cells.is_empty() {
            return false;
        }
        let (columns, rows) = span(rect);
        for column in columns {
            for row in rows.clone() {
                let Some(held) = self.cells.get(&(column, row)) else {
                    continue;
                };
                let found = held.iter().any(|&index| {
                    let index = index as usize;
                    admits(index) && inks.get(index).is_some_and(|ink| rect.intersects(*ink))
                });
                if found {
                    return true;
                }
            }
        }
        false
    }
}

/// Returns the columns and rows of the cells `rect` covers.
fn span(
    rect: DeviceRect,
) -> (
    core::ops::RangeInclusive<i32>,
    core::ops::RangeInclusive<i32>,
) {
    let cell = |value: i32| value.div_euclid(CELL).clamp(LOWEST, HIGHEST);
    (
        cell(rect.left())..=cell(rect.right()),
        cell(rect.top())..=cell(rect.bottom()),
    )
}

#[cfg(test)]
mod tests {
    use iceblink::DeviceRect;
    use iceblink::geom::{Point, Size};

    use super::InkCells;

    fn rect(x: i32, y: i32, width: i32, height: i32) -> DeviceRect {
        DeviceRect::new(Point::new(x, y), Size::new(width, height))
    }

    #[test]
    fn a_rectangle_meets_only_the_inks_it_overlaps() {
        let inks = [rect(0, 0, 10, 10), rect(500, 500, 10, 10)];
        let mut cells = InkCells::default();
        for (index, ink) in inks.iter().enumerate() {
            cells.insert(index, *ink);
        }
        assert!(cells.meets(rect(5, 5, 2, 2), &inks, |_| true));
        assert!(!cells.meets(rect(20, 20, 2, 2), &inks, |_| true));
        assert!(cells.meets(rect(505, 505, 50, 50), &inks, |_| true));
        assert!(!cells.meets(rect(505, 505, 50, 50), &inks, |index| index == 0));
    }

    #[test]
    fn ink_far_off_the_output_is_still_found() {
        let inks = [rect(-90_000, 40, 100_000, 10)];
        let mut cells = InkCells::default();
        cells.insert(0, inks[0]);
        assert!(cells.meets(rect(-80_000, 42, 5, 5), &inks, |_| true));
        assert!(cells.meets(rect(3000, 42, 5, 5), &inks, |_| true));
    }
}
