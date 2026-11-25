//! Atlas pages held in memory.

use iceblink::{DeviceRect, PageFormat, PageOp};

/// An atlas page held in memory, with its texels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    /// The texel format.
    pub format: PageFormat,
    /// The width and height, in texels.
    pub size: [u32; 2],
    /// The texels, row by row with no padding.
    pub texels: Vec<u8>,
    /// The levels of detail below this one, from level 1.
    pub below: Vec<Page>,
}

impl Page {
    /// Returns a page of transparent texels with one level.
    pub fn new(format: PageFormat, size: [u32; 2]) -> Self {
        let texels = size[0] as usize * size[1] as usize * texel_bytes(format);
        Self {
            format,
            size,
            texels: vec![0; texels],
            below: Vec::new(),
        }
    }

    /// Returns a page of transparent texels with `levels` levels.
    pub fn with_levels(format: PageFormat, size: [u32; 2], levels: u32) -> Self {
        let mut page = Self::new(format, size);
        page.below = (1..levels)
            .map(|level| Self::new(format, PageOp::level_size(size, level)))
            .collect();
        page
    }

    /// Returns level `level`, or `None` past the last level.
    pub fn level(&self, level: u32) -> Option<&Page> {
        match level {
            0 => Some(self),
            _ => self.below.get(level as usize - 1),
        }
    }

    /// Copies the texels of `rect` of level `level` as [`Page::upload`] does.
    ///
    /// Nothing is copied past the last level.
    pub fn upload_level(
        &mut self,
        level: u32,
        rect: DeviceRect,
        bytes: &[u8],
        offset: usize,
        stride: u32,
    ) {
        match level {
            0 => self.upload(rect, bytes, offset, stride),
            _ => {
                if let Some(below) = self.below.get_mut(level as usize - 1) {
                    below.upload(rect, bytes, offset, stride);
                }
            }
        }
    }

    /// Copies the texels of `rect` from `bytes`, starting at `offset` and `stride` bytes apart.
    ///
    /// The parts of `rect` outside the page, and the rows `bytes` does not hold, are left out.
    pub fn upload(&mut self, rect: DeviceRect, bytes: &[u8], offset: usize, stride: u32) {
        let texel = texel_bytes(self.format);
        let left = rect.origin.x.max(0) as usize;
        let right = (rect.origin.x + rect.size.width).clamp(0, self.size[0] as i32) as usize;
        let top = rect.origin.y.max(0) as usize;
        let bottom = (rect.origin.y + rect.size.height).clamp(0, self.size[1] as i32) as usize;
        if left >= right || top >= bottom {
            return;
        }

        let skip = (left as i64 - i64::from(rect.origin.x)) as usize * texel;
        let row = (right - left) * texel;
        let pitch = self.size[0] as usize * texel;
        for y in top..bottom {
            let line = (y as i64 - i64::from(rect.origin.y)) as usize;
            let from = offset + line * stride as usize + skip;
            let Some(source) = bytes.get(from..from + row) else {
                return;
            };
            let to = y * pitch + left * texel;
            self.texels[to..to + row].copy_from_slice(source);
        }
    }

    /// Returns the bytes of the texel at `(x, y)`, or `None` outside the page.
    pub fn texel(&self, x: u32, y: u32) -> Option<&[u8]> {
        if x >= self.size[0] || y >= self.size[1] {
            return None;
        }

        let texel = texel_bytes(self.format);
        let at = (y as usize * self.size[0] as usize + x as usize) * texel;
        self.texels.get(at..at + texel)
    }
}

/// Returns how many bytes one texel of `format` takes.
pub fn texel_bytes(format: PageFormat) -> usize {
    if format == PageFormat::A8 { 1 } else { 4 }
}
