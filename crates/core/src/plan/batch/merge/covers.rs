//! Opaque covers taken from quad batches.

use crate::plan::batch::merge::Sequences;
use crate::plan::cover::{self, Covers};
use crate::plan::emission::{LaneKind, NO_COVER};

use iceblink::OpaqueCover;

/// Keeps the covers of the quads at draw positions `range`, drawn by batch `batch`.
pub(super) fn collect(
    sequences: &Sequences<'_>,
    range: core::ops::Range<usize>,
    batch: u32,
    covers: &mut Covers,
) {
    let quads = sequences.records.lane(LaneKind::Quads);
    for at in range {
        let index = quads.placed_at(at).cover;
        if index == NO_COVER {
            continue;
        }
        cover::keep(
            covers,
            OpaqueCover {
                rect: sequences.covers[index as usize],
                batch,
                at: at as u32,
            },
        );
    }
}
