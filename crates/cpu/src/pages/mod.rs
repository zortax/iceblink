//! Atlas pages as the renderer holds them: texels per level, created, written and destroyed by
//! the frames' page operations.

mod sample;

use iceblink::{PageFormat, PageOp};

pub(crate) use self::sample::{nearest, trilinear};

/// One level of a page: its size and its texels, row by row.
#[derive(Clone, Debug)]
pub(crate) struct Level {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) texels: Vec<u8>,
}

/// One atlas page.
#[derive(Clone, Debug)]
pub(crate) struct Page {
    pub(crate) format: PageFormat,
    pub(crate) levels: Vec<Level>,
}

impl Page {
    /// Returns the bytes one texel of the page takes.
    pub(crate) fn texel_bytes(&self) -> usize {
        bytes_per_texel(self.format)
    }

    /// Returns texel `(x, y)` of `level` as four channels from zero to one, with an `A8` texel in
    /// the first channel.
    ///
    /// A texel past the level's edge reads as the nearest edge texel.
    pub(crate) fn load(&self, level: usize, x: i32, y: i32) -> [f32; 4] {
        let Some(held) = self.levels.get(level) else {
            return [0.0; 4];
        };
        if held.width == 0 || held.height == 0 {
            return [0.0; 4];
        }
        let x = x.clamp(0, held.width as i32 - 1) as usize;
        let y = y.clamp(0, held.height as i32 - 1) as usize;
        let bytes = self.texel_bytes();
        let at = (y * held.width as usize + x) * bytes;
        let unit = |byte: u8| f32::from(byte) / 255.0;
        if bytes == 1 {
            return [unit(held.texels[at]), 0.0, 0.0, 1.0];
        }
        [
            unit(held.texels[at]),
            unit(held.texels[at + 1]),
            unit(held.texels[at + 2]),
            unit(held.texels[at + 3]),
        ]
    }

    /// Returns the size of level 0.
    pub(crate) fn size(&self) -> [u32; 2] {
        self.levels
            .first()
            .map_or([0, 0], |level| [level.width, level.height])
    }

    fn bytes(&self) -> u64 {
        self.levels
            .iter()
            .map(|level| level.texels.len() as u64)
            .sum()
    }
}

/// Every page the renderer holds, by slot.
#[derive(Debug, Default)]
pub(crate) struct Pages {
    pages: Vec<Option<Page>>,
}

impl Pages {
    /// Applies the frame's page operations in order, and returns the bytes they wrote.
    ///
    /// Created pages are clamped to `max_size` texels a side. An upload or destroy of a page the
    /// renderer does not hold does nothing. An upload that extends past its level writes what lies
    /// inside, and one whose bytes end early writes the rows they hold.
    pub(crate) fn apply(&mut self, ops: &[PageOp], max_size: u32) -> u64 {
        let mut written = 0;
        for op in ops {
            match op {
                PageOp::Create {
                    page,
                    format,
                    size,
                    levels,
                } => self.create(*page, *format, *size, *levels, max_size),
                PageOp::Upload {
                    page,
                    level,
                    rect,
                    bytes,
                    offset,
                    stride,
                } => {
                    if let Some(held) = self.get_mut(*page) {
                        written += upload(held, *level, *rect, bytes, *offset, *stride);
                    }
                }
                PageOp::Destroy { page } => {
                    if let Some(held) = self.pages.get_mut(*page as usize) {
                        *held = None;
                    }
                }
            }
        }
        written
    }

    /// Returns the page in `slot`, if the renderer holds one.
    pub(crate) fn get(&self, slot: u32) -> Option<&Page> {
        self.pages.get(slot as usize).and_then(Option::as_ref)
    }

    /// Returns the bytes every page holds.
    pub(crate) fn bytes(&self) -> u64 {
        self.pages.iter().flatten().map(Page::bytes).sum()
    }

    fn get_mut(&mut self, slot: u32) -> Option<&mut Page> {
        self.pages.get_mut(slot as usize).and_then(Option::as_mut)
    }

    fn create(&mut self, slot: u32, format: PageFormat, size: [u32; 2], levels: u32, max: u32) {
        let size = [size[0].min(max), size[1].min(max)];
        let most = 32 - size[0].max(size[1]).max(1).leading_zeros();
        let bytes = bytes_per_texel(format);
        let levels = (0..levels.clamp(1, most))
            .map(|level| {
                let [width, height] = PageOp::level_size(size, level);
                Level {
                    width,
                    height,
                    texels: vec![0; width as usize * height as usize * bytes],
                }
            })
            .collect();
        let index = slot as usize;
        if self.pages.len() <= index {
            self.pages.resize_with(index + 1, || None);
        }
        self.pages[index] = Some(Page { format, levels });
    }
}

fn bytes_per_texel(format: PageFormat) -> usize {
    if format == PageFormat::A8 { 1 } else { 4 }
}

fn upload(
    page: &mut Page,
    level: u32,
    rect: iceblink::DeviceRect,
    bytes: &[u8],
    offset: usize,
    stride: u32,
) -> u64 {
    let texel = page.texel_bytes();
    let Some(held) = page.levels.get_mut(level as usize) else {
        return 0;
    };
    let left = rect.origin.x.max(0);
    let top = rect.origin.y.max(0);
    let right = (rect.origin.x + rect.size.width).min(held.width as i32);
    let bottom = (rect.origin.y + rect.size.height).min(held.height as i32);
    if right <= left || bottom <= top {
        return 0;
    }
    let mut written = 0;
    for y in top..bottom {
        let source_row = offset + (y - rect.origin.y) as usize * stride as usize;
        let start = source_row + (left - rect.origin.x) as usize * texel;
        let length = (right - left) as usize * texel;
        let Some(source) = bytes.get(start..start + length) else {
            break;
        };
        let at = (y as usize * held.width as usize + left as usize) * texel;
        held.texels[at..at + length].copy_from_slice(source);
        written += length as u64;
    }
    written
}
