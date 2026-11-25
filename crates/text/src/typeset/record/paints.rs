//! Paint references of one chunk.

use iceblink::{ChunkRecorder, PaintId, PaintRef};

use crate::typeset::style::TextPaint;

/// A cache of the paint references recorded into a chunk, by paint slot.
#[derive(Default)]
pub(super) struct Paints {
    slots: Vec<(PaintId, PaintRef)>,
}

impl Paints {
    /// Returns the paint reference that items of `chunk` store for `paint`.
    ///
    /// A paint slot is referenced in the chunk on its first use only.
    pub(super) fn resolve(&mut self, chunk: &mut ChunkRecorder<'_>, paint: TextPaint) -> PaintRef {
        match paint {
            TextPaint::Color(rgba) => PaintRef::color(rgba),
            TextPaint::Slot(kind, id) => {
                if let Some(&(_, reference)) = self.slots.iter().find(|(slot, _)| *slot == id) {
                    return reference;
                }
                let reference = chunk.paint_ref(kind, id);
                self.slots.push((id, reference));
                reference
            }
        }
    }
}
