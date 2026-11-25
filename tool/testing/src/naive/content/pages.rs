//! Atlas pages the compositor owns, with a copy of every texel.

use std::collections::BTreeMap;
use std::sync::Arc;

use iceblink::PageFormat;

use crate::naive::content::shelf::Shelves;

/// The texels between tiles on a shared page, so a tile sampled bilinearly never reads another
/// tile's texels.
const TILE_GAP: u32 = 1;

/// The kind of content a page holds.
///
/// It decides which cache entries are dropped when the page is evicted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PageUse {
    /// Glyph tiles, shared by many glyphs.
    Glyphs,
    /// The texels of one image, with its levels.
    Image,
    /// Mark shape cells, shared by many payloads.
    Marks,
    /// Path masks, shared by many paths.
    Masks,
}

/// One page and every level of its texels.
pub(crate) struct Page {
    pub(crate) format: PageFormat,
    pub(crate) size: [u32; 2],
    pub(crate) purpose: PageUse,
    /// The texels of each level, row after row without padding.
    levels: Vec<Vec<u8>>,
    /// A shared copy of each level, made when a renderer needs it and dropped by a write.
    copies: Vec<Option<Arc<[u8]>>>,
    /// A version that changes with every write.
    pub(crate) version: u64,
    /// The free space of a shared page.
    pub(crate) shelves: Option<Shelves>,
    /// The build counter of the last frame that drew from the page.
    pub(crate) last_drawn: u64,
}

impl Page {
    pub(crate) fn bytes(&self) -> u64 {
        self.levels.iter().map(|level| level.len() as u64).sum()
    }

    pub(crate) fn level_count(&self) -> u32 {
        self.levels.len() as u32
    }

    /// Returns the texels of `level`, shared with the uploads that carry them.
    pub(crate) fn level(&mut self, level: u32) -> Arc<[u8]> {
        let level = level as usize;
        self.copies[level]
            .get_or_insert_with(|| self.levels[level].as_slice().into())
            .clone()
    }
}

/// Every page, by id.
#[derive(Default)]
pub(crate) struct Pages {
    pub(crate) pages: BTreeMap<u32, Page>,
    next_id: u32,
    versions: u64,
}

impl Pages {
    /// Adds a page whose levels hold `levels`, and returns its id.
    ///
    /// A `shared` page gets free space that tiles are placed into.
    pub(crate) fn create(
        &mut self,
        format: PageFormat,
        size: [u32; 2],
        purpose: PageUse,
        levels: Vec<Vec<u8>>,
        shared: bool,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.versions += 1;
        let shelves = shared.then(|| Shelves::new(size, TILE_GAP));
        self.pages.insert(
            id,
            Page {
                format,
                size,
                purpose,
                copies: vec![None; levels.len()],
                levels,
                version: self.versions,
                shelves,
                last_drawn: 0,
            },
        );
        id
    }

    /// Adds a shared page of transparent texels with one level.
    pub(crate) fn create_shared(&mut self, format: PageFormat, side: u32, purpose: PageUse) -> u32 {
        let bytes = side as usize * side as usize * bytes_per_texel(format);
        self.create(format, [side, side], purpose, vec![vec![0; bytes]], true)
    }

    /// Places a `width` by `height` tile on a shared page of `format` and `purpose`, creating a
    /// page of `side` texels when no page has room, and returns the page and the tile's corner.
    ///
    /// Returns `None` when the tile is larger than a page.
    pub(crate) fn place(
        &mut self,
        format: PageFormat,
        purpose: PageUse,
        side: u32,
        size: [u32; 2],
    ) -> Option<(u32, [u32; 2])> {
        for (id, page) in self.pages.iter_mut().rev() {
            if page.format != format || page.purpose != purpose {
                continue;
            }
            if let Some(at) = page
                .shelves
                .as_mut()
                .and_then(|shelves| shelves.place(size))
            {
                return Some((*id, at));
            }
        }

        let mut shelves = Shelves::new([side, side], TILE_GAP);
        let at = shelves.place(size)?;
        let id = self.create_shared(format, side, purpose);
        self.pages.get_mut(&id)?.shelves = Some(shelves);
        Some((id, at))
    }

    /// Places several tiles on one shared page, as `place` places one.
    ///
    /// Returns `None` when the tiles fit on no page together.
    pub(crate) fn place_all(
        &mut self,
        format: PageFormat,
        purpose: PageUse,
        side: u32,
        sizes: &[[u32; 2]],
    ) -> Option<(u32, Vec<[u32; 2]>)> {
        let fits = |shelves: &Shelves| -> Option<(Shelves, Vec<[u32; 2]>)> {
            let mut trial = shelves.clone();
            let corners = sizes
                .iter()
                .map(|size| trial.place(*size))
                .collect::<Option<Vec<_>>>()?;
            Some((trial, corners))
        };

        for (id, page) in self.pages.iter_mut().rev() {
            if page.format != format || page.purpose != purpose {
                continue;
            }
            let Some(shelves) = page.shelves.as_mut() else {
                continue;
            };
            if let Some((placed, corners)) = fits(shelves) {
                *shelves = placed;
                return Some((*id, corners));
            }
        }

        let (placed, corners) = fits(&Shelves::new([side, side], TILE_GAP))?;
        let id = self.create_shared(format, side, purpose);
        self.pages.get_mut(&id)?.shelves = Some(placed);
        Some((id, corners))
    }

    /// Copies `texels`, rows of `width` texels, into level 0 of `page` with its corner at `at`.
    pub(crate) fn write(&mut self, page: u32, at: [u32; 2], width: u32, texels: &[u8]) {
        let Some(page) = self.pages.get_mut(&page) else {
            return;
        };

        let per = bytes_per_texel(page.format);
        let row = width as usize * per;
        let stride = page.size[0] as usize * per;
        let level = &mut page.levels[0];
        for (y, source) in texels.chunks_exact(row.max(1)).enumerate() {
            let start = (at[1] as usize + y) * stride + at[0] as usize * per;
            level[start..start + row].copy_from_slice(source);
        }

        page.copies[0] = None;
        self.versions += 1;
        page.version = self.versions;
    }

    /// Removes `page`, and returns its bytes.
    pub(crate) fn destroy(&mut self, page: u32) -> u64 {
        self.pages.remove(&page).map_or(0, |page| page.bytes())
    }

    pub(crate) fn bytes(&self) -> u64 {
        self.pages.values().map(Page::bytes).sum()
    }
}

pub(crate) fn bytes_per_texel(format: PageFormat) -> usize {
    if format == PageFormat::A8 { 1 } else { 4 }
}
