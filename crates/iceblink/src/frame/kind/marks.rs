//! Marks with their atlas cells.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

use crate::item::{MarkPayload, Marks};

/// A marks item, with the atlas page that holds its payload's shape cells.
///
/// The item is drawn as the `Marks` item describes. Its `payload` indexes the mark payloads of
/// the resident chunk it comes from, or `Lanes::mark_payloads` when it is transient. Each
/// placement reads its cell from `page` at the phase of its anchor.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct FrameMarks {
    /// The item, with frame slots.
    pub marks: Marks,
    /// The atlas page that holds the payload's shape cells.
    ///
    /// It is read only when the instance draws a placement whose shape has a cell of nonzero size.
    pub page: u32,
}

/// A marks payload with the atlas cells of its shapes.
#[derive(Clone, Debug)]
pub struct FrameMarkPayload {
    /// The payload whose shapes the cells hold.
    pub payload: Arc<MarkPayload>,
    /// Sixteen cells per shape, indexed `shape × 16 + phase_y × 4 + phase_x`.
    pub cells: Arc<[MarkCell]>,
}

/// One shape of a marks payload, rasterised at one phase.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct MarkCell {
    /// The cell, in texels of the payload's page, as `[x, y, width, height]`.
    pub rect: [u32; 4],
    /// Where the cell starts from the anchor's pixel, in device pixels.
    pub offset: [i32; 2],
}
