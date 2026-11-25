//! Image texels on pages of their own, with levels of detail.

use std::collections::HashMap;

use iceblink::{ClientId, PageFormat};

use crate::naive::content::pages::{PageUse, Pages};
use crate::naive::scene::StoredImage;

/// The page that holds an image's texels.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ImagePage {
    pub(crate) page: u32,
    /// The image version the page holds.
    pub(crate) version: u64,
    /// How many image texels each page texel averages on each axis.
    pub(crate) reduced: u32,
    /// The size of level 0.
    pub(crate) size: [u32; 2],
}

/// The page of every image that a frame drew, by client and slot.
#[derive(Default)]
pub(crate) struct Images {
    pages: HashMap<(ClientId, u32), ImagePage>,
}

impl Images {
    /// Returns the page of `image`, building it again when the image has changed.
    ///
    /// An image with a side longer than `max_side` is averaged down to fit. Returns `None` for an
    /// image with no texels.
    pub(crate) fn page(
        &mut self,
        pages: &mut Pages,
        key: (ClientId, u32),
        image: &StoredImage,
        max_side: u32,
    ) -> Option<ImagePage> {
        if let Some(held) = self.pages.get(&key) {
            if held.version == image.version {
                return Some(*held);
            }
            pages.destroy(held.page);
            self.pages.remove(&key);
        }

        let [width, height] = [image.desc.width, image.desc.height];
        if width == 0 || height == 0 {
            return None;
        }

        let reduced = width.max(height).div_ceil(max_side.max(1));
        let (size, base) = reduce(&image.texels, [width, height], reduced);
        let levels = levels(base, size);
        let page = pages.create(
            PageFormat::RGBA8_PREMULTIPLIED,
            size,
            PageUse::Image,
            levels,
            false,
        );

        let held = ImagePage {
            page,
            version: image.version,
            reduced,
            size,
        };
        self.pages.insert(key, held);
        Some(held)
    }

    /// Destroys the pages of images for which `exists` returns false, given the image's key and
    /// version.
    pub(crate) fn drop_missing(
        &mut self,
        pages: &mut Pages,
        exists: impl Fn((ClientId, u32), u64) -> bool,
    ) {
        self.pages.retain(|key, held| {
            let keep = exists(*key, held.version);
            if !keep {
                pages.destroy(held.page);
            }
            keep
        });
    }

    /// Forgets the image whose page is `page`.
    pub(crate) fn evict(&mut self, page: u32) {
        self.pages.retain(|_, held| held.page != page);
    }
}

/// Averages blocks of `factor` by `factor` texels, and returns the reduced size and texels.
fn reduce(texels: &[u8], size: [u32; 2], factor: u32) -> ([u32; 2], Vec<u8>) {
    if factor <= 1 {
        return (size, texels.to_vec());
    }

    let [width, height] = size.map(|side| side as usize);
    let factor = factor as usize;
    let reduced = [width.div_ceil(factor), height.div_ceil(factor)];
    let mut out = Vec::with_capacity(reduced[0] * reduced[1] * 4);
    for y in 0..reduced[1] {
        for x in 0..reduced[0] {
            let mut sum = [0u32; 4];
            let mut count = 0u32;
            for sy in y * factor..((y + 1) * factor).min(height) {
                for sx in x * factor..((x + 1) * factor).min(width) {
                    let at = (sy * width + sx) * 4;
                    for channel in 0..4 {
                        sum[channel] += u32::from(texels[at + channel]);
                    }
                    count += 1;
                }
            }
            out.extend(sum.map(|total| ((total + count / 2) / count) as u8));
        }
    }

    ([reduced[0] as u32, reduced[1] as u32], out)
}

/// Returns level 0 and every smaller level down to one texel.
///
/// Each level is a 2×2 box filter of the level above. An odd last row or column is left out, and
/// a side of one texel is read twice.
fn levels(base: Vec<u8>, size: [u32; 2]) -> Vec<Vec<u8>> {
    let count = 32 - size[0].max(size[1]).leading_zeros();
    let mut levels = vec![base];
    let mut size = size.map(|side| side as usize);
    for _ in 1..count {
        let above = levels.last().expect("level 0 exists");
        let next = [(size[0] / 2).max(1), (size[1] / 2).max(1)];
        let mut level = Vec::with_capacity(next[0] * next[1] * 4);
        for y in 0..next[1] {
            for x in 0..next[0] {
                let xs = [(2 * x).min(size[0] - 1), (2 * x + 1).min(size[0] - 1)];
                let ys = [(2 * y).min(size[1] - 1), (2 * y + 1).min(size[1] - 1)];
                for channel in 0..4 {
                    let mut sum = 2u32;
                    for sy in ys {
                        for sx in xs {
                            sum += u32::from(above[(sy * size[0] + sx) * 4 + channel]);
                        }
                    }
                    level.push((sum / 4) as u8);
                }
            }
        }
        levels.push(level);
        size = next;
    }

    levels
}
