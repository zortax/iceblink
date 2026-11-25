//! Where the sixteen phase cells of mark shapes lie in a sheet.

/// How many phases a pixel is divided into along each axis.
pub(crate) const PHASES: usize = 4;

/// How many cells one shape has.
pub(crate) const CELLS: usize = PHASES * PHASES;

/// One phase of one shape in a sheet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Cell {
    /// Where the cell starts in the sheet, in texels.
    pub(crate) at: [u32; 2],
    /// Its extent, in texels.
    pub(crate) size: [u32; 2],
    /// Where it starts from the anchor's pixel.
    pub(crate) offset: [i32; 2],
}

impl Cell {
    /// Returns the shift of cell `index` from the anchor, in device pixels.
    pub(crate) fn shift(index: usize) -> [f32; 2] {
        [
            (index % PHASES) as f32 / PHASES as f32,
            (index / PHASES) as f32 / PHASES as f32,
        ]
    }
}

/// Returns the cells of a shape whose control box is `bounds`, and the extent of the four by
/// four grid that holds them, or `None` when a cell is not finite or wider or taller than
/// `max_cell`.
///
/// Cell `4 × phase_y + phase_x` covers the shape moved by `(phase_x, phase_y) / 4`, rounded out
/// to whole pixels and at least one pixel on each side. Every cell of the grid takes the extent
/// of the largest one.
pub(crate) fn shape_cells(bounds: kurbo::Rect, max_cell: u32) -> Option<([Cell; CELLS], [u32; 2])> {
    let mut cells = [Cell::default(); CELLS];
    let mut largest = [1u32, 1u32];
    for (index, cell) in cells.iter_mut().enumerate() {
        let [dx, dy] = Cell::shift(index).map(f64::from);
        let left = (bounds.x0 + dx).floor();
        let top = (bounds.y0 + dy).floor();
        let right = (bounds.x1 + dx).ceil();
        let bottom = (bounds.y1 + dy).ceil();
        if ![left, top, right, bottom]
            .iter()
            .all(|edge| edge.is_finite())
        {
            return None;
        }
        let width = (right - left).max(1.0);
        let height = (bottom - top).max(1.0);
        if width > f64::from(max_cell) || height > f64::from(max_cell) {
            return None;
        }

        cell.size = [width as u32, height as u32];
        cell.offset = [left as i32, top as i32];
        largest = [largest[0].max(cell.size[0]), largest[1].max(cell.size[1])];
    }

    for (index, cell) in cells.iter_mut().enumerate() {
        cell.at = [
            (index % PHASES) as u32 * largest[0],
            (index / PHASES) as u32 * largest[1],
        ];
    }

    Some((
        cells,
        [largest[0] * PHASES as u32, largest[1] * PHASES as u32],
    ))
}

/// Places grids of the given extents in rows no wider than `row_limit`, and returns the corner
/// of each grid and the extent of the sheet.
///
/// A grid wider than `row_limit` gets a row of its own.
pub(crate) fn pack(grids: &[[u32; 2]], row_limit: u32) -> (Vec<[u32; 2]>, [u32; 2]) {
    let mut corners = Vec::with_capacity(grids.len());
    let mut cursor = [0u32, 0u32];
    let mut row_height = 0u32;
    let mut width = 0u32;
    for grid in grids {
        if cursor[0] > 0 && cursor[0] + grid[0] > row_limit {
            cursor = [0, cursor[1] + row_height];
            row_height = 0;
        }
        corners.push(cursor);
        cursor[0] += grid[0];
        row_height = row_height.max(grid[1]);
        width = width.max(cursor[0]);
    }
    (corners, [width.max(1), (cursor[1] + row_height).max(1)])
}
