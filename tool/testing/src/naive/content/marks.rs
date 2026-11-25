//! Mark shapes rasterised into cells at sixteen phases.

use std::collections::HashMap;
use std::sync::Arc;

use iceblink::kurbo::{BezPath, PathEl};
use iceblink::{MarkCell, MarkPayload, PageFormat};
use zeno::{Command, Mask, Placement, Vector};

use crate::naive::content::pages::{PageUse, Pages};

/// The cells of a payload's shapes.
#[derive(Clone, Debug)]
pub(crate) struct MarkCells {
    /// The page every cell lies on, when any cell has texels.
    pub(crate) page: Option<u32>,
    /// Sixteen cells per shape, indexed `shape × 16 + phase_y × 4 + phase_x`.
    pub(crate) cells: Arc<[MarkCell]>,
    /// Whether each shape is too large for a cell, and so has empty cells and is drawn as a path.
    pub(crate) oversize: Arc<[bool]>,
}

/// The cells of every payload a frame drew, by the address of the payload.
#[derive(Default)]
pub(crate) struct Marks {
    /// Each payload and its cells, by the payload's address.
    // Holding the payload keeps its address from being reused while the cells are kept.
    held: HashMap<usize, (Arc<MarkPayload>, MarkCells)>,
}

impl Marks {
    /// Returns the cells of `payload`, rasterising them onto a page of `side` texels the first
    /// time.
    ///
    /// A shape whose cells are wider or taller than a quarter of the page side is oversize.
    pub(crate) fn cells(
        &mut self,
        pages: &mut Pages,
        side: u32,
        payload: &Arc<MarkPayload>,
    ) -> MarkCells {
        let address = Arc::as_ptr(payload) as usize;
        if let Some((_, cells)) = self.held.get(&address) {
            return cells.clone();
        }

        let cells = rasterise(pages, side, payload);
        self.held.insert(address, (payload.clone(), cells.clone()));
        cells
    }

    /// Forgets every payload whose cells lie on `page`.
    pub(crate) fn evict(&mut self, page: u32) {
        self.held.retain(|_, (_, cells)| cells.page != Some(page));
    }
}

const EMPTY: Placement = Placement {
    left: 0,
    top: 0,
    width: 0,
    height: 0,
};

/// A rendered mask and where it lies from the shape's origin.
type PhaseMask = (Vec<u8>, Placement);

/// Rasterises the cells of every shape of `payload` onto one shared page of `side` texels.
fn rasterise(pages: &mut Pages, side: u32, payload: &MarkPayload) -> MarkCells {
    let (masks, oversize) = phase_masks(payload, side / 4);

    let sizes: Vec<[u32; 2]> = masks
        .iter()
        .filter(|(_, placement)| placement.width > 0 && placement.height > 0)
        .map(|(_, placement)| [placement.width, placement.height])
        .collect();
    let placed = if sizes.is_empty() {
        None
    } else {
        pages.place_all(PageFormat::A8, PageUse::Marks, side, &sizes)
    };

    // Cells that fit on no page together are drawn as paths, like oversize shapes.
    if placed.is_none() && !sizes.is_empty() {
        let cells = vec![MarkCell::default(); masks.len()];
        return MarkCells {
            page: None,
            cells: cells.into(),
            oversize: vec![true; payload.shapes.len()].into(),
        };
    }

    let mut corners = placed.as_ref().map(|(_, corners)| corners.iter());
    let cells = masks
        .iter()
        .map(|(texels, placement)| {
            if placement.width == 0 || placement.height == 0 {
                return MarkCell::default();
            }
            let (Some((page, _)), Some(at)) = (&placed, corners.as_mut().and_then(Iterator::next))
            else {
                return MarkCell::default();
            };
            pages.write(*page, *at, placement.width, texels);
            MarkCell {
                rect: [at[0], at[1], placement.width, placement.height],
                offset: [placement.left, placement.top],
            }
        })
        .collect();

    MarkCells {
        page: placed.map(|(page, _)| page),
        cells,
        oversize: oversize.into(),
    }
}

/// Renders the sixteen phase masks of every shape, and reports which shapes are oversize: a mask
/// wider or taller than `limit`.
///
/// An oversize shape gets sixteen empty masks.
fn phase_masks(payload: &MarkPayload, limit: u32) -> (Vec<PhaseMask>, Vec<bool>) {
    let mut masks = Vec::with_capacity(payload.shapes.len() * 16);
    let mut oversize = Vec::with_capacity(payload.shapes.len());
    for shape in &payload.shapes {
        let start = masks.len();
        for phase in 0..16u32 {
            let shift = [(phase % 4) as f32 / 4.0, (phase / 4) as f32 / 4.0];
            let commands = commands(shape, shift);
            masks.push(Mask::new(&commands).render());
        }

        let large = masks[start..]
            .iter()
            .any(|(_, placement)| placement.width > limit || placement.height > limit);
        if large {
            masks.truncate(start);
            masks.extend((0..16).map(|_| (Vec::new(), EMPTY)));
        }
        oversize.push(large);
    }

    (masks, oversize)
}

/// Returns the commands of `path` moved by `shift`.
fn commands(path: &BezPath, shift: [f32; 2]) -> Vec<Command> {
    let point =
        |p: iceblink::kurbo::Point| Vector::new(p.x as f32 + shift[0], p.y as f32 + shift[1]);
    path.elements()
        .iter()
        .map(|element| match *element {
            PathEl::MoveTo(p) => Command::MoveTo(point(p)),
            PathEl::LineTo(p) => Command::LineTo(point(p)),
            PathEl::QuadTo(a, b) => Command::QuadTo(point(a), point(b)),
            PathEl::CurveTo(a, b, c) => Command::CurveTo(point(a), point(b), point(c)),
            PathEl::ClosePath => Command::Close,
        })
        .collect()
}
