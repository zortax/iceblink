//! The pages the recorded page operations have created and not destroyed.

use std::collections::BTreeMap;
use std::sync::Arc;

use iceblink::geom::{Point, Rect, Size};
use iceblink::{DeviceRect, PageFormat, PageOp};

/// One page that exists.
#[derive(Debug)]
struct HeldPage {
    format: PageFormat,
    size: [u32; 2],
    levels: u32,
    /// The texels of level 0, row by row, for a page whose content cannot be built again.
    texels: Option<Vec<u8>>,
}

/// The pages that exist, with a copy of the texels of each page that keeps one.
#[derive(Debug, Default)]
pub(crate) struct HeldPages {
    pages: BTreeMap<u32, HeldPage>,
    copied_bytes: u64,
}

impl HeldPages {
    /// Records the creation of a page of transparent texels, which keeps a copy of its texels
    /// when `copied` is set.
    pub(crate) fn create(
        &mut self,
        page: u32,
        format: PageFormat,
        size: [u32; 2],
        levels: u32,
        copied: bool,
    ) {
        let texels = copied.then(|| vec![0; texel_count(size) * texel_bytes(format)]);
        self.copied_bytes += texels.as_ref().map_or(0, |texels| texels.len() as u64);

        let previous = self.pages.insert(
            page,
            HeldPage {
                format,
                size,
                levels,
                texels,
            },
        );
        self.forget(previous);
    }

    /// Copies the texels of `rect` of level `level` of `page` from `bytes`, starting at `offset`
    /// and `stride` bytes apart, into the page's copy.
    pub(crate) fn write(
        &mut self,
        page: u32,
        level: u32,
        rect: DeviceRect,
        bytes: &[u8],
        offset: usize,
        stride: usize,
    ) {
        let Some(held) = self.pages.get_mut(&page) else {
            return;
        };
        let Some(texels) = held.texels.as_mut().filter(|_| level == 0) else {
            return;
        };

        let texel = texel_bytes(held.format);
        let [x, y] = [rect.origin.x.max(0) as usize, rect.origin.y.max(0) as usize];
        let [width, height] = [
            rect.size.width.max(0) as usize,
            rect.size.height.max(0) as usize,
        ];
        let pitch = held.size[0] as usize * texel;
        let row = width.min((held.size[0] as usize).saturating_sub(x)) * texel;
        let rows = height.min((held.size[1] as usize).saturating_sub(y));

        for line in 0..rows {
            let from = offset + line * stride;
            let Some(source) = bytes.get(from..from + row) else {
                return;
            };
            let to = (y + line) * pitch + x * texel;
            texels[to..to + row].copy_from_slice(source);
        }
    }

    /// Records the destruction of `page`.
    pub(crate) fn destroy(&mut self, page: u32) {
        let previous = self.pages.remove(&page);
        self.forget(previous);
    }

    /// Appends a creation of every page, in page order, followed by an upload of the whole
    /// copy of each page that keeps one.
    pub(crate) fn replay(&self, out: &mut Vec<PageOp>) {
        for (&page, held) in &self.pages {
            out.push(PageOp::Create {
                page,
                format: held.format,
                size: held.size,
                levels: held.levels,
            });
            let Some(texels) = &held.texels else {
                continue;
            };
            out.push(PageOp::Upload {
                page,
                level: 0,
                rect: Rect::new(
                    Point::new(0, 0),
                    Size::new(held.size[0] as i32, held.size[1] as i32),
                ),
                bytes: Arc::from(texels.as_slice()),
                offset: 0,
                stride: held.size[0] * texel_bytes(held.format) as u32,
            });
        }
    }

    /// Returns the bytes of the copies.
    pub(crate) fn copied_bytes(&self) -> u64 {
        self.copied_bytes
    }

    fn forget(&mut self, page: Option<HeldPage>) {
        let bytes = page
            .and_then(|page| page.texels)
            .map_or(0, |texels| texels.len() as u64);
        self.copied_bytes -= bytes;
    }
}

fn texel_count(size: [u32; 2]) -> usize {
    size[0] as usize * size[1] as usize
}

fn texel_bytes(format: PageFormat) -> usize {
    if format == PageFormat::A8 { 1 } else { 4 }
}
