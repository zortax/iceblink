//! Lowered chunks a renderer keeps on its device.

use std::sync::Arc;

use crate::frame::{ColorSprite, FrameMarkPayload, FrameMarks, FrameShadow, Sprite};
use crate::item::{Line, Quad, ShaderQuad};

/// A change to the lowered chunks a renderer keeps resident.
///
/// A revision identifies one immutable lowered chunk and is unique for the lifetime of the output.
#[derive(Clone, Debug)]
pub enum ChunkNote {
    /// A lowered chunk came into existence.
    Inserted {
        /// The revision that identifies the new chunk.
        revision: u64,
        /// The new lowered chunk.
        chunk: Arc<LoweredChunk>,
    },
    /// A lowered chunk will not be drawn again.
    Retired {
        /// The revision of the retired chunk.
        revision: u64,
    },
    /// Every resident chunk is gone.
    ResetAll,
}

/// A chunk's instances, one array per frame kind, at the offset they were lowered at.
#[derive(Clone, Debug, Default)]
pub struct LoweredChunk {
    /// Quads.
    pub quads: Vec<Quad>,
    /// Shader quads.
    pub shader_quads: Vec<ShaderQuad>,
    /// Shadows.
    pub shadows: Vec<FrameShadow>,
    /// Lines.
    pub lines: Vec<Line>,
    /// Mono sprites.
    pub mono: Vec<Sprite>,
    /// Subpixel sprites.
    pub subpixel: Vec<Sprite>,
    /// Colour sprites.
    pub color: Vec<ColorSprite>,
    /// Marks.
    pub marks: Vec<FrameMarks>,
    /// The payloads the marks index.
    pub mark_payloads: Vec<FrameMarkPayload>,
}

/// A resident chunk a frame draws, and how far it is moved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawnChunk {
    /// The revision of the resident chunk drawn.
    pub revision: u64,
    /// The offset added to the lowered instances, in each instance's local units.
    pub offset: [f32; 2],
}
