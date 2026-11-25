//! Checking a marks item against its payload.

use iceblink::{MarkFlags, Marks};

use super::Check;
use crate::commit::finite;

impl Check<'_> {
    /// Checks a marks item's payload index, its counts and caps, and the prims it draws.
    ///
    /// The prims drawn must be finite, and each placement drawn must refer to a finite shape of
    /// the payload.
    pub(super) fn marks(&mut self, item: &Marks) {
        let caps = [item.flags.start_cap(), item.flags.end_cap()];
        self.valid(caps.iter().all(|&cap| cap <= MarkFlags::CAP_ROUND));
        let counts = [item.discs, item.boxes, item.vertices, item.placements];
        let nonzero = counts.iter().filter(|&&count| count > 0).count();
        self.valid(nonzero == 1 || item.first == 0);

        let Some(payload) = self.chunk.payloads.get(item.payload as usize) else {
            self.valid(false);
            return;
        };
        let first = item.first as usize;
        let drawn = |count: u32, len: usize| match count {
            0 => Some(0..0),
            _ => (first + count as usize <= len).then_some(first..first + count as usize),
        };
        let (Some(discs), Some(boxes), Some(vertices), Some(placements)) = (
            drawn(item.discs, payload.discs.len()),
            drawn(item.boxes, payload.boxes.len()),
            drawn(item.vertices, payload.vertices.len()),
            drawn(item.placements, payload.placements.len()),
        ) else {
            self.valid(false);
            return;
        };

        self.valid(payload.discs[discs].iter().all(finite::all));
        self.valid(
            payload.boxes[boxes]
                .iter()
                .all(|mark| finite::all(mark.rect.iter().chain(&mark.radii).chain(&mark.shape))),
        );
        // A separator's NaN `x` is the one non-finite value a vertex may hold.
        self.valid(
            payload.vertices[vertices]
                .iter()
                .all(|&[x, y]| x.is_nan() || (x.is_finite() && y.is_finite())),
        );
        let named = payload.placements[placements.clone()]
            .iter()
            .all(|placement| {
                finite::all([&placement.x, &placement.y])
                    && (placement.shape as usize) < payload.shapes.len()
            });
        self.valid(named);
        if !placements.is_empty() {
            self.valid(payload.shapes.iter().all(|shape| finite::path(shape)));
        }
    }
}
