//! Appending items.

use crate::item::{
    Backdrop, ExternalQuad, GlyphRun, Image, ItemKind, ItemOp, Line, Marks, PathItem, Quad,
    ShaderQuad, Shadow,
};
use crate::transaction::RecordedChunk;
use crate::transaction::recorder::{ChunkRecorder, index_of_push};

impl ChunkRecorder<'_> {
    /// Appends a quad.
    pub fn quad(&mut self, item: Quad) -> u32 {
        self.push(ItemKind::QUAD, item, |chunk| &mut chunk.quads)
    }

    /// Appends a shader quad.
    pub fn shader_quad(&mut self, item: ShaderQuad) -> u32 {
        self.push(ItemKind::SHADER_QUAD, item, |chunk| &mut chunk.shader_quads)
    }

    /// Appends a shadow.
    pub fn shadow(&mut self, item: Shadow) -> u32 {
        self.push(ItemKind::SHADOW, item, |chunk| &mut chunk.shadows)
    }

    /// Appends a decoration line.
    pub fn line(&mut self, item: Line) -> u32 {
        self.push(ItemKind::LINE, item, |chunk| &mut chunk.lines)
    }

    /// Appends a glyph run, whose glyphs and coordinates are ranges of the chunk's arenas.
    pub fn glyph_run(&mut self, item: GlyphRun) -> u32 {
        self.push(ItemKind::GLYPH_RUN, item, |chunk| &mut chunk.glyph_runs)
    }

    /// Appends an image.
    pub fn image(&mut self, item: Image) -> u32 {
        self.push(ItemKind::IMAGE, item, |chunk| &mut chunk.images)
    }

    /// Appends a marks item, whose payload is an index of the chunk's payload list.
    pub fn marks(&mut self, item: Marks) -> u32 {
        self.push(ItemKind::MARKS, item, |chunk| &mut chunk.marks)
    }

    /// Appends a path.
    pub fn path(&mut self, item: PathItem) -> u32 {
        self.push(ItemKind::PATH, item, |chunk| &mut chunk.paths)
    }

    /// Appends an external texture.
    pub fn external(&mut self, item: ExternalQuad) -> u32 {
        self.push(ItemKind::EXTERNAL, item, |chunk| &mut chunk.externals)
    }

    /// Appends a backdrop, whose filters are an index of the chunk's filter-chain list.
    pub fn backdrop(&mut self, item: Backdrop) -> u32 {
        self.push(ItemKind::BACKDROP, item, |chunk| &mut chunk.backdrops)
    }

    fn push<T>(
        &mut self,
        kind: ItemKind,
        item: T,
        array: fn(&mut RecordedChunk) -> &mut Vec<T>,
    ) -> u32 {
        // Items go straight into the per-kind arrays in their final layout, so each is copied
        // once.
        let index = index_of_push(array(self.chunk), item);
        index_of_push(
            &mut self.chunk.ops,
            ItemOp {
                kind,
                _pad: [0; 3],
                index,
            },
        )
    }
}
