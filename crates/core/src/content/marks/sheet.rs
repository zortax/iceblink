//! Sheets: every phase of a payload's shapes, rasterised into one tile.

use std::sync::Arc;

use iceblink::MarkCell;
use kurbo::{BezPath, PathEl};

use crate::content::coverage::{Style, commands_of, raster};
use crate::content::marks::layout::{CELLS, Cell, pack, shape_cells};

/// The exact geometry of a payload's shapes, which a sheet is kept by.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SheetKey(Box<[u32]>);

impl SheetKey {
    /// Returns the key of `shapes`, from the `f32` bits of every point.
    pub(crate) fn of(shapes: &[Arc<BezPath>]) -> Self {
        let mut words = Vec::new();
        let push = |words: &mut Vec<u32>, point: &kurbo::Point| {
            words.push((point.x as f32).to_bits());
            words.push((point.y as f32).to_bits());
        };
        for shape in shapes {
            for element in shape.elements() {
                match element {
                    PathEl::MoveTo(p) => {
                        words.push(0);
                        push(&mut words, p);
                    }
                    PathEl::LineTo(p) => {
                        words.push(1);
                        push(&mut words, p);
                    }
                    PathEl::QuadTo(a, b) => {
                        words.push(2);
                        push(&mut words, a);
                        push(&mut words, b);
                    }
                    PathEl::CurveTo(a, b, c) => {
                        words.push(3);
                        push(&mut words, a);
                        push(&mut words, b);
                        push(&mut words, c);
                    }
                    PathEl::ClosePath => words.push(4),
                }
            }
            words.push(u32::MAX);
        }
        Self(words.into_boxed_slice())
    }
}

/// A sheet before it has a tile: its texels, its extent and each cell from its corner.
pub(crate) struct Sheet {
    pub(crate) size: [u32; 2],
    pub(crate) texels: Vec<u8>,
    /// Sixteen cells per shape, in the order a frame indexes them.
    pub(crate) cells: Vec<Cell>,
    /// Whether each shape is too large for a cell, so its cells are empty.
    pub(crate) large: Vec<bool>,
}

/// Lays out and rasterises the sheet of `shapes`.
///
/// Each shape is filled by the non-zero rule at every quarter-pixel phase on each axis. A shape
/// whose cell would be wider or taller than `max_cell` gets sixteen empty cells.
pub(crate) fn rasterise(shapes: &[Arc<BezPath>], max_cell: u32, row_limit: u32) -> Sheet {
    let layouts: Vec<Option<_>> = shapes
        .iter()
        .map(|shape| shape_cells(shape.control_box(), max_cell))
        .collect();
    let grids: Vec<[u32; 2]> = layouts
        .iter()
        .map(|layout| layout.as_ref().map_or([0, 0], |(_, grid)| *grid))
        .collect();
    let (corners, size) = pack(&grids, row_limit);

    let width = size[0] as usize;
    let mut texels = vec![0u8; width * size[1] as usize];
    let mut cells = Vec::with_capacity(shapes.len() * CELLS);
    let mut commands = Vec::new();
    let style = Style::Fill(false);
    for ((shape, layout), corner) in shapes.iter().zip(&layouts).zip(&corners) {
        let Some((shape_cells, _)) = layout else {
            cells.extend([Cell::default(); CELLS]);
            continue;
        };
        for (index, cell) in shape_cells.iter().enumerate() {
            let [dx, dy] = Cell::shift(index);
            let shift = [dx - cell.offset[0] as f32, dy - cell.offset[1] as f32];
            commands.clear();
            commands_of(
                shape,
                |point| [point.x as f32 + shift[0], point.y as f32 + shift[1]],
                &mut commands,
            );
            let coverage = raster(&commands, &style, cell.size);

            let at = [corner[0] + cell.at[0], corner[1] + cell.at[1]];
            let row = cell.size[0] as usize;
            for (line, values) in coverage.chunks_exact(row.max(1)).enumerate() {
                let start = (at[1] as usize + line) * width + at[0] as usize;
                texels[start..start + row].copy_from_slice(values);
            }
            cells.push(Cell { at, ..*cell });
        }
    }
    Sheet {
        size,
        texels,
        cells,
        large: layouts.iter().map(Option::is_none).collect(),
    }
}

/// Returns the frame cells of a sheet whose tile starts at `origin` on its page.
pub(crate) fn frame_cells(cells: &[Cell], origin: [u32; 2]) -> Arc<[MarkCell]> {
    cells
        .iter()
        .map(|cell| MarkCell {
            rect: [
                origin[0] + cell.at[0],
                origin[1] + cell.at[1],
                cell.size[0],
                cell.size[1],
            ],
            offset: cell.offset,
        })
        .collect()
}
