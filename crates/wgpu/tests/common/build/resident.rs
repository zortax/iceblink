//! Resident chunks inserted, retired and drawn at offsets.

use std::sync::Arc;

use iceblink::{ChunkNote, DrawnChunk, ItemSource, LoweredChunk, Quad};

use super::FrameBuilder;
use super::batches::Entry;

impl FrameBuilder {
    /// Inserts `chunk` as resident revision `revision`, without drawing it.
    pub(crate) fn insert(&mut self, revision: u64, chunk: Arc<LoweredChunk>) {
        self.frame
            .chunk_notes
            .push(ChunkNote::Inserted { revision, chunk });
    }

    /// Retires resident revision `revision`.
    pub(crate) fn retire(&mut self, revision: u64) {
        self.frame.chunk_notes.push(ChunkNote::Retired { revision });
    }

    /// Draws every quad of resident revision `revision`, moved by `offset`.
    ///
    /// `quads` holds the chunk's quads, which the inks are measured from.
    pub(crate) fn draw_resident_quads(&mut self, revision: u64, offset: [f32; 2], quads: &[Quad]) {
        let chunk = self.frame.drawn.len() as u32;
        self.frame.drawn.push(DrawnChunk { revision, offset });
        for (index, quad) in quads.iter().enumerate() {
            let [x, y, w, h] = quad.bounds;
            let ink = self.ink([x + offset[0], y + offset[1], w, h], quad.space.0);
            self.frame
                .lanes
                .quads
                .sources
                .push(ItemSource::resident(chunk, index as u32));
            self.entries.push((Entry::Quad, ink));
        }
    }

    /// Draws every instance of resident revision `revision`, moved by `offset`.
    ///
    /// The lanes are drawn in the order quads, shader quads, shadows, lines, mono, subpixel and
    /// colour sprites, and marks.
    pub(crate) fn draw_resident(&mut self, revision: u64, offset: [f32; 2], chunk: &LoweredChunk) {
        let index = self.frame.drawn.len() as u32;
        self.frame.drawn.push(DrawnChunk { revision, offset });
        let moved = |[x, y, w, h]: [f32; 4]| [x + offset[0], y + offset[1], w, h];
        let lanes = &mut self.frame.lanes;
        let mut entries = Vec::new();

        for (at, quad) in chunk.quads.iter().enumerate() {
            lanes
                .quads
                .sources
                .push(ItemSource::resident(index, at as u32));
            entries.push((Entry::Quad, moved(quad.bounds), quad.space.0));
        }
        for (at, quad) in chunk.shader_quads.iter().enumerate() {
            lanes
                .shader_quads
                .sources
                .push(ItemSource::resident(index, at as u32));
            let entry = Entry::ShaderQuad(quad.shader, quad.params);
            entries.push((entry, moved(quad.bounds), quad.space.0));
        }
        for (at, shadow) in chunk.shadows.iter().enumerate() {
            lanes
                .shadows
                .sources
                .push(ItemSource::resident(index, at as u32));
            entries.push((Entry::Shadow, moved(shadow.bounds), shadow.shadow.space.0));
        }
        for (at, line) in chunk.lines.iter().enumerate() {
            lanes
                .lines
                .sources
                .push(ItemSource::resident(index, at as u32));
            entries.push((Entry::Line, moved(line.bounds), line.space.0));
        }
        for (at, sprite) in chunk.mono.iter().enumerate() {
            lanes
                .mono
                .sources
                .push(ItemSource::resident(index, at as u32));
            entries.push((
                Entry::Mono(sprite.tile.page),
                moved(sprite.bounds),
                sprite.space,
            ));
        }
        for (at, sprite) in chunk.subpixel.iter().enumerate() {
            lanes
                .subpixel
                .sources
                .push(ItemSource::resident(index, at as u32));
            let entry = Entry::Subpixel(sprite.tile.page);
            entries.push((entry, moved(sprite.bounds), sprite.space));
        }
        for (at, sprite) in chunk.color.iter().enumerate() {
            lanes
                .color
                .sources
                .push(ItemSource::resident(index, at as u32));
            entries.push((
                Entry::Color(sprite.tile.page),
                moved(sprite.frame),
                sprite.space,
            ));
        }
        for (at, marks) in chunk.marks.iter().enumerate() {
            lanes
                .marks
                .sources
                .push(ItemSource::resident(index, at as u32));
            entries.push((Entry::Marks, moved(marks.marks.bounds), marks.marks.space.0));
        }

        for (entry, bounds, space) in entries {
            let ink = self.ink(bounds, space);
            self.entries.push((entry, ink));
        }
    }
}
