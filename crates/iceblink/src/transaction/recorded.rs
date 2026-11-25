//! Recorded chunks.

use std::sync::Arc;

use crate::chunk::{ChunkKey, ChunkProps, LocalClip};
use crate::geometry::LocalRect;
use crate::id::AnyId;
use crate::item::{
    Backdrop, ExternalQuad, Glyph, GlyphRun, Image, ItemOp, Line, MarkPayload, Marks, PathItem,
    Quad, ShaderQuad, Shadow,
};
use crate::node::Filter;
use crate::transaction::ChunkRecorder;

/// One recorded chunk: one array per item kind, the record order across them, and the chunk's
/// arenas and lists.
///
/// Each item array is indexed by the `ops` entries of its kind.
#[derive(Clone, Debug)]
pub struct RecordedChunk {
    /// The key the chunk is recorded under.
    pub key: ChunkKey,
    /// The chunk's default property state.
    pub props: ChunkProps,
    /// Every item in record order, as its kind and its index in that kind's array.
    ///
    /// An item's position here is its index in the chunk.
    pub ops: Vec<ItemOp>,
    /// The quads.
    pub quads: Vec<Quad>,
    /// The shader quads.
    pub shader_quads: Vec<ShaderQuad>,
    /// The shadows.
    pub shadows: Vec<Shadow>,
    /// The decoration lines.
    pub lines: Vec<Line>,
    /// The glyph runs.
    pub glyph_runs: Vec<GlyphRun>,
    /// The images.
    pub images: Vec<Image>,
    /// The marks items.
    pub marks: Vec<Marks>,
    /// The paths.
    pub paths: Vec<PathItem>,
    /// The external textures.
    pub externals: Vec<ExternalQuad>,
    /// The backdrops.
    pub backdrops: Vec<Backdrop>,
    /// The glyph arena, which glyph runs take ranges of.
    pub glyphs: Vec<Glyph>,
    /// The coordinate arena, which glyph runs take ranges of, in F2Dot14.
    pub coords: Vec<i16>,
    /// The payload list, which marks items index.
    pub payloads: Vec<Arc<MarkPayload>>,
    /// The filter-chain list, which backdrops index.
    pub filters: Vec<Arc<[Filter]>>,
    /// The chunk-local clips, which `ClipRef::local` values index.
    pub local_clips: Vec<LocalClip>,
    /// The rectangle outside which re-recording the chunk changed nothing, in the chunk's space.
    pub hint: Option<LocalRect>,
    /// The ids the items' slots were taken from, whose generations the commit checks.
    pub refs: Vec<AnyId>,
}

impl RecordedChunk {
    pub(crate) fn new(key: ChunkKey, props: ChunkProps) -> Self {
        Self {
            key,
            props,
            ops: Vec::new(),
            quads: Vec::new(),
            shader_quads: Vec::new(),
            shadows: Vec::new(),
            lines: Vec::new(),
            glyph_runs: Vec::new(),
            images: Vec::new(),
            marks: Vec::new(),
            paths: Vec::new(),
            externals: Vec::new(),
            backdrops: Vec::new(),
            glyphs: Vec::new(),
            coords: Vec::new(),
            payloads: Vec::new(),
            filters: Vec::new(),
            local_clips: Vec::new(),
            hint: None,
            refs: Vec::new(),
        }
    }

    /// Returns a recorder that appends items to the chunk.
    pub fn recorder(&mut self) -> ChunkRecorder<'_> {
        ChunkRecorder::new(self)
    }

    /// Empties every array and list, keeping their capacity.
    pub fn clear(&mut self) {
        self.ops.clear();
        self.quads.clear();
        self.shader_quads.clear();
        self.shadows.clear();
        self.lines.clear();
        self.glyph_runs.clear();
        self.images.clear();
        self.marks.clear();
        self.paths.clear();
        self.externals.clear();
        self.backdrops.clear();
        self.glyphs.clear();
        self.coords.clear();
        self.payloads.clear();
        self.filters.clear();
        self.local_clips.clear();
        self.hint = None;
        self.refs.clear();
    }
}
