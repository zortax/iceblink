//! Whether paints are opaque solid colours.

use iceblink::{Paint, PaintKind, PaintRef};

use crate::scene::record::PaintRecord;
use crate::scene::slab::Slab;

/// Reports whether a client paint reference paints an opaque solid colour.
pub(crate) fn opaque_ref(paints: &Slab<PaintRecord>, paint: PaintRef) -> bool {
    match paint.inline_color() {
        Some([.., alpha]) => alpha == u8::MAX,
        None => {
            paint.kind == PaintKind::SOLID
                && paints
                    .at(paint.index)
                    .is_some_and(|record| opaque(&record.paint))
        }
    }
}

/// Reports whether a paint is an opaque solid colour.
fn opaque(paint: &Paint) -> bool {
    matches!(paint, Paint::Solid(color) if color.is_opaque())
}
