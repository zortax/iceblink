//! A lowering under construction.

use std::sync::Arc;

use iceblink::{FramePath, Instance, LoweredChunk};
use smallvec::SmallVec;

use crate::content::{Lowered as ContentLowered, TileKey};
use crate::scene::chunk::{
    Checked, ContentStamp, Lane, Lowered, LoweredItem, LoweredPath, LoweringKey, Part, Placement,
};
use crate::shared_content::Content;

/// The arrays, parts and atlas inputs of a lowering being made.
pub(crate) struct Build {
    /// The chunk offset the instances are lowered at.
    pub(crate) base: [f32; 2],
    pub(crate) chunk: LoweredChunk,
    pub(crate) items: Vec<LoweredItem>,
    pub(crate) parts: Vec<Part>,
    pub(crate) paths: Vec<LoweredPath>,
    pub(crate) tiles: Vec<TileKey>,
    pub(crate) worlds: SmallVec<[Placement; 1]>,
    /// Whether any item lowered into atlas content.
    pub(crate) atlas: bool,
}

impl Build {
    pub(crate) fn new(base: [f32; 2], items: usize) -> Self {
        Self {
            base,
            chunk: LoweredChunk::default(),
            items: Vec::with_capacity(items),
            parts: Vec::new(),
            paths: Vec::new(),
            tiles: Vec::new(),
            worlds: SmallVec::new(),
            atlas: false,
        }
    }

    /// Records that content was placed under `placement`.
    ///
    /// One placement is kept per client space. It is phased when any content placed sprites under
    /// that space.
    pub(crate) fn placed_under(&mut self, placement: Placement) {
        match self
            .worlds
            .iter_mut()
            .find(|held| held.slot == placement.slot)
        {
            Some(held) => held.phased |= placement.phased,
            None => self.worlds.push(placement),
        }
    }

    /// Appends the instances content lowering made for one item drawn through the frame clip
    /// slot `clip` in client space `space`, and returns where they went.
    pub(crate) fn content(
        &mut self,
        lowered: &mut Vec<ContentLowered>,
        clip: u32,
        space: u32,
    ) -> LoweredItem {
        let start = self.parts.len() as u32;
        for item in lowered.drain(..) {
            match item {
                ContentLowered::Mono(sprite) => self.part(Lane::Mono, |chunk| {
                    chunk.mono.push(sprite);
                    chunk.mono.len()
                }),
                ContentLowered::Subpixel(sprite) => self.part(Lane::Subpixel, |chunk| {
                    chunk.subpixel.push(sprite);
                    chunk.subpixel.len()
                }),
                ContentLowered::Color(sprite) => self.part(Lane::Color, |chunk| {
                    chunk.color.push(sprite);
                    chunk.color.len()
                }),
                ContentLowered::Path(path) => self.path(path, clip, space),
            }
        }

        let len = self.parts.len() as u32 - start;
        match self.parts.get(start as usize..) {
            None | Some([]) => LoweredItem::Nothing,
            Some(&[Part::Instances { lane, start, len }]) => {
                self.parts.pop();
                LoweredItem::Instances { lane, start, len }
            }
            Some(_) => LoweredItem::Parts { start, len },
        }
    }

    /// Appends a frame path drawn through the frame clip slot `clip` in client space `space` as
    /// the next part.
    pub(crate) fn path(&mut self, path: FramePath, clip: u32, space: u32) {
        self.parts.push(Part::Path(self.paths.len() as u32));
        self.paths.push(LoweredPath { path, clip, space });
    }

    /// Adds one instance pushed by `push`, which returns the new length of its array, to the
    /// last part when that part is of the same lane.
    fn part(&mut self, lane: Lane, push: impl FnOnce(&mut LoweredChunk) -> usize) {
        let at = push(&mut self.chunk) as u32 - 1;
        if let Some(Part::Instances {
            lane: last,
            start,
            len,
        }) = self.parts.last_mut()
            && *last == lane
            && *start + *len == at
        {
            *len += 1;
            return;
        }

        self.parts.push(Part::Instances {
            lane,
            start: at,
            len: 1,
        });
    }

    /// Returns the lowering made for `key`, with a content stamp when any item lowered into atlas
    /// content.
    ///
    /// The lowering has revision 0 and holds no combined clip chains.
    pub(crate) fn finish(self, key: LoweringKey, content: &Content) -> Lowered {
        let stamp = self.atlas.then(|| ContentStamp {
            epoch: content.epoch,
            checked: Checked {
                placements: content.placements,
                evictions: content.cache.evictions(),
                offset: self.base,
            },
            tiles: self.tiles.into(),
            resident: false,
            worlds: self.worlds,
            rerouted: false,
        });

        Lowered {
            revision: 0,
            key,
            base: self.base,
            chunk: Arc::new(self.chunk),
            items: shared(self.items),
            parts: shared(self.parts),
            paths: shared(self.paths),
            content: stamp,
            combined: Box::default(),
        }
    }
}

/// Returns `values` as a shared slice.
fn shared<T>(values: Vec<T>) -> Arc<[T]> {
    // An empty default slice is not allocated.
    if values.is_empty() {
        Arc::default()
    } else {
        values.into()
    }
}

/// Pushes one instance moved by `base` onto `array`, and returns where it went.
pub(crate) fn push<T: Instance>(
    array: &mut Vec<T>,
    lane: Lane,
    value: T,
    base: [f32; 2],
) -> LoweredItem {
    let start = array.len() as u32;
    let value = if base == [0.0; 2] {
        value
    } else {
        value.moved(base)
    };
    array.push(value);
    LoweredItem::Instances {
        lane,
        start,
        len: 1,
    }
}
