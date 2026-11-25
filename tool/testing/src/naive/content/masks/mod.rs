//! Path masks rasterised onto atlas pages, and the histories that route paths onto them.

mod raster;
mod routes;

use std::collections::HashMap;

use iceblink::{AtlasTile, PageFormat};

use crate::naive::content::pages::{PageUse, Pages};

pub(crate) use self::routes::{PathId, Routes};

/// The coverage geometry of one part of a path: its outline in mask texels from the mask's
/// corner, how the outline is drawn and the mask's size.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Mask {
    pub(crate) commands: Vec<MaskCommand>,
    pub(crate) style: MaskStyle,
    pub(crate) size: [u32; 2],
}

/// One outline command, with every point as the `f32` bits of its texel position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum MaskCommand {
    Move([u32; 2]),
    Line([u32; 2]),
    Quad([u32; 4]),
    Curve([u32; 6]),
    Close,
}

/// How a mask's outline is drawn, with every length as the `f32` bits of a length in texels.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum MaskStyle {
    /// The interior, by the even-odd rule when set and the non-zero rule otherwise.
    Fill { even_odd: bool },
    /// The outline a stroke draws.
    Stroke {
        width: u32,
        join: StrokeJoin,
        /// The miter limit, a ratio to the width.
        miter_limit: u32,
        /// The start and end caps.
        caps: [StrokeCap; 2],
        dashes: Vec<u32>,
        dash_offset: u32,
    },
}

/// How a stroke joins its segments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum StrokeJoin {
    Bevel,
    Miter,
    Round,
}

/// How a stroke ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum StrokeCap {
    Butt,
    Square,
    Round,
}

impl Mask {
    /// Returns the number of texels the mask covers.
    pub(crate) fn texels(&self) -> u64 {
        u64::from(self.size[0]) * u64::from(self.size[1])
    }
}

/// Mask tiles, kept until their page is evicted or the caches are dropped.
#[derive(Default)]
pub(crate) struct MaskTiles {
    tiles: HashMap<Mask, AtlasTile>,
}

impl MaskTiles {
    /// Returns the tile of `mask`, rasterising it onto a page of `side` texels the first time,
    /// or `None` when it fits no page.
    pub(crate) fn tile(&mut self, pages: &mut Pages, side: u32, mask: &Mask) -> Option<AtlasTile> {
        if let Some(tile) = self.tiles.get(mask) {
            return Some(*tile);
        }

        let (page, at) = pages.place(PageFormat::A8, PageUse::Masks, side, mask.size)?;
        pages.write(page, at, mask.size[0], &raster::coverage(mask));
        let tile = AtlasTile {
            page,
            rect: [at[0], at[1], mask.size[0], mask.size[1]],
        };
        self.tiles.insert(mask.clone(), tile);
        Some(tile)
    }

    /// Forgets every tile on `page`.
    pub(crate) fn evict(&mut self, page: u32) {
        self.tiles.retain(|_, tile| tile.page != page);
    }
}
