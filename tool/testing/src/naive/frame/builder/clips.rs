//! Frame clip slots and the links they hold.

use iceblink::smallvec::SmallVec;
use iceblink::{AtlasTile, ClipEntry, ClipLinkEntry, FRAME_NO_CLIP, FrameClipShape};

use crate::naive::frame::builder::Builder;
use crate::naive::frame::chain::{Link, LinkShape};

impl Builder<'_> {
    /// Returns the frame clip slot of a chain, or `FRAME_NO_CLIP` for an empty one.
    pub(crate) fn clip(&mut self, chain: &[Link]) -> u32 {
        if chain.is_empty() {
            return FRAME_NO_CLIP;
        }

        let key: Vec<_> = chain.iter().map(|link| link.id.clone()).collect();
        let (frame_slot, first) = self.slots.clips.slot(&key);
        if first {
            let mut links = SmallVec::new();
            for link in chain {
                if let Some(entry) = self.link(link) {
                    links.push(entry);
                }
            }
            self.clips.set(frame_slot, ClipEntry { links });
        }
        frame_slot
    }

    fn link(&mut self, link: &Link) -> Option<ClipLinkEntry> {
        let (space, _) = self.space(link.space)?;
        let shape = match &link.shape {
            LinkShape::Rounded {
                rect,
                radii,
                shapes,
            } => FrameClipShape::RoundedRect {
                rect: *rect,
                radii: *radii,
                shapes: *shapes,
            },
            LinkShape::Mask {
                rect,
                image,
                source,
                mode,
            } => FrameClipShape::Mask {
                rect: *rect,
                tile: self.mask_tile(*image, *source)?,
                mode: *mode,
            },
        };
        Some(ClipLinkEntry { space, shape })
    }

    /// Returns the part of an image's tile that `source` covers, rounded to whole texels.
    fn mask_tile(&mut self, image: u32, source: [f32; 4]) -> Option<AtlasTile> {
        let placed = self.image_page(image)?;
        let scale = 1.0 / placed.reduced as f32;
        let [width, height] = placed.size.map(|side| side as f32);
        let x0 = (source[0] * scale).round().clamp(0.0, width);
        let y0 = (source[1] * scale).round().clamp(0.0, height);
        let x1 = ((source[0] + source[2]) * scale).round().clamp(x0, width);
        let y1 = ((source[1] + source[3]) * scale).round().clamp(y0, height);
        Some(AtlasTile {
            page: placed.page,
            rect: [x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32],
        })
    }
}
