//! Stories: sequences of transaction steps that build a picture, and the world they share.

use iceblink::{
    ChunkKey, ClipId, Color, EffectId, FontId, ImageId, OrderEntry, OrderId, Paint, PaintId,
    PaintKind, ScrollId, Transaction, TransformId,
};

use crate::common::props;

/// One step of a story, written into a transaction.
pub(crate) type Step = fn(&mut Transaction, &mut World);

/// A named sequence of steps.
#[derive(Clone, Copy)]
pub(crate) struct Story {
    pub(crate) name: &'static str,
    pub(crate) steps: &'static [Step],
}

/// The ids a story's steps create and later steps refer to.
#[derive(Debug)]
pub(crate) struct World {
    pub(crate) root: OrderId,
    pub(crate) space: TransformId,
    pub(crate) font: FontId,
    /// How many entries the root's sequence holds.
    pub(crate) entries: u32,
    pub(crate) paints: Vec<PaintId>,
    pub(crate) transforms: Vec<TransformId>,
    pub(crate) scrolls: Vec<ScrollId>,
    pub(crate) clips: Vec<ClipId>,
    pub(crate) effects: Vec<EffectId>,
    pub(crate) orders: Vec<OrderId>,
    pub(crate) images: Vec<ImageId>,
}

impl World {
    pub(crate) fn new(root: OrderId, space: TransformId, font: FontId) -> Self {
        Self {
            root,
            space,
            font,
            entries: 0,
            paints: Vec::new(),
            transforms: Vec::new(),
            scrolls: Vec::new(),
            clips: Vec::new(),
            effects: Vec::new(),
            orders: Vec::new(),
            images: Vec::new(),
        }
    }

    /// Creates a solid paint and remembers it.
    pub(crate) fn solid(&mut self, tx: &mut Transaction, rgba: [f32; 4]) -> PaintId {
        let paint = tx.create_paint(Paint::Solid(Color(rgba)));
        self.paints.push(paint);
        paint
    }

    /// Appends an entry to the root's sequence.
    pub(crate) fn append(&mut self, tx: &mut Transaction, entry: OrderEntry) {
        tx.splice(self.root, self.entries, 0, &[entry]);
        self.entries += 1;
    }

    /// Records chunk `key` in the root space with quads filled by `paint`, and appends it to the
    /// root if `append` is set.
    pub(crate) fn boxes(
        &mut self,
        tx: &mut Transaction,
        key: ChunkKey,
        paint: PaintId,
        rects: &[[f32; 4]],
        append: bool,
    ) {
        let mut chunk = tx.record(key, props(self.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        for &rect in rects {
            chunk.quad(crate::common::quad(rect, fill));
        }
        if append {
            self.append(tx, OrderEntry::Chunk(key));
        }
    }
}
