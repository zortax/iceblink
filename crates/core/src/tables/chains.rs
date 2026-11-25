//! Clip chains vector planning asks for: the chain of a clip without its innermost links.

use iceblink::{ClipEntry, ClipLinkEntry, FRAME_NO_CLIP, FrameClipShape};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::tables::table::FrameTable;

/// The number of interned chains above which chains no frame used are released.
const KEEP: usize = 256;

/// Frame clip slots made for chains by their links, so equal chains share a slot.
///
/// An interned chain's parent is the interned chain without its innermost link. The slot of a
/// chain never changes while the chain is interned, since its links are its key.
#[derive(Debug, Default)]
pub(crate) struct ChainIntern {
    slots: FxHashMap<Vec<u32>, u32>,
    /// The parent and the key of every interned slot.
    interned: FxHashMap<u32, (u32, Vec<u32>)>,
    /// The parents of the chains a frame's paths are drawn through.
    frame: FxHashMap<u32, u32>,
    /// The build serial of the frame that last used each interned slot.
    used: FxHashMap<u32, u64>,
}

impl ChainIntern {
    /// Starts the parents of a new frame.
    pub(crate) fn begin(&mut self) {
        self.frame.clear();
    }

    /// Records the parent of the chain `clip` for the frame being planned.
    pub(crate) fn note(&mut self, clips: &mut FrameTable<ClipEntry>, clip: u32, serial: u64) {
        if clip == FRAME_NO_CLIP
            || self.interned.contains_key(&clip)
            || self.frame.contains_key(&clip)
        {
            if self.interned.contains_key(&clip) {
                self.touch(clip, serial);
            }
            return;
        }

        let links: SmallVec<[ClipLinkEntry; 2]> = clips
            .get(clip)
            .map(|entry| entry.links.clone())
            .unwrap_or_default();
        let parent = match links.len() {
            0 | 1 => FRAME_NO_CLIP,
            len => self.intern(clips, &links[..len - 1], serial),
        };
        self.frame.insert(clip, parent);
    }

    /// Returns the parent of a chain noted or interned, or the empty chain for any other.
    pub(crate) fn parent(&self, clip: u32) -> u32 {
        if let Some(&parent) = self.frame.get(&clip) {
            return parent;
        }
        self.interned
            .get(&clip)
            .map_or(FRAME_NO_CLIP, |&(parent, _)| parent)
    }

    /// Returns the slot of the chain of `links`, interning it and its parents as needed.
    pub(crate) fn intern(
        &mut self,
        clips: &mut FrameTable<ClipEntry>,
        links: &[ClipLinkEntry],
        serial: u64,
    ) -> u32 {
        if links.is_empty() {
            return FRAME_NO_CLIP;
        }

        let key = key_of(links);
        if let Some(&slot) = self.slots.get(&key) {
            self.touch(slot, serial);
            return slot;
        }

        let parent = self.intern(clips, &links[..links.len() - 1], serial);
        let slot = clips.allocate(ClipEntry {
            links: links.iter().cloned().collect(),
        });
        self.slots.insert(key.clone(), slot);
        self.interned.insert(slot, (parent, key));
        self.used.insert(slot, serial);
        slot
    }

    /// Marks an interned slot and its parents used by the frame of `serial`.
    fn touch(&mut self, slot: u32, serial: u64) {
        let mut current = slot;
        while let Some(&(parent, _)) = self.interned.get(&current) {
            self.used.insert(current, serial);
            current = parent;
        }
    }

    /// Releases the interned chains the frame of `serial` did not use, once there are many.
    pub(crate) fn trim(&mut self, clips: &mut FrameTable<ClipEntry>, serial: u64) {
        if self.interned.len() <= KEEP {
            return;
        }

        let unused: Vec<u32> = self
            .used
            .iter()
            .filter(|&(_, &at)| at != serial)
            .map(|(&slot, _)| slot)
            .collect();
        for slot in unused {
            self.used.remove(&slot);
            if let Some((_, key)) = self.interned.remove(&slot) {
                self.slots.remove(&key);
            }
            clips.release(slot, serial);
        }
    }
}

/// Returns the words that identify a chain by its links.
fn key_of(links: &[ClipLinkEntry]) -> Vec<u32> {
    let mut key = Vec::with_capacity(links.len() * 16);

    for link in links {
        key.push(link.space);
        match &link.shape {
            FrameClipShape::RoundedRect {
                rect,
                radii,
                shapes,
            } => {
                key.push(0);
                key.extend(
                    rect.iter()
                        .chain(radii)
                        .chain(shapes)
                        .map(|value| value.to_bits()),
                );
            }
            FrameClipShape::Mask { rect, tile, mode } => {
                key.push(1);
                key.extend(rect.iter().map(|value| value.to_bits()));
                key.push(tile.page);
                key.extend(tile.rect);
                key.push(u32::from(mode.0));
            }
        }
    }

    key
}
