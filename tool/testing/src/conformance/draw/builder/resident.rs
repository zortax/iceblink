//! The resident chunks a frame inserts, retires and draws.

use std::sync::Arc;

use iceblink::{ChunkNote, DrawnChunk, ItemSource, Lane, LoweredChunk, Quad};

use crate::conformance::draw::builder::FrameBuilder;
use crate::conformance::draw::builder::batches::Entry;

impl FrameBuilder {
    /// Inserts `chunk` as resident revision `revision`, without drawing it.
    pub fn insert(&mut self, revision: u64, chunk: Arc<LoweredChunk>) {
        self.frame
            .chunk_notes
            .push(ChunkNote::Inserted { revision, chunk });
    }

    /// Retires resident revision `revision`.
    pub fn retire(&mut self, revision: u64) {
        self.frame.chunk_notes.push(ChunkNote::Retired { revision });
    }

    /// Draws every quad of resident revision `revision`, moved by `offset`.
    ///
    /// The batch inks are those of `quads`, the chunk's quads.
    pub fn draw_resident_quads(&mut self, revision: u64, offset: [f32; 2], quads: &[Quad]) {
        let chunk = self.draw_chunk(revision, offset);
        push_resident(&mut self.frame.lanes.quads, chunk, quads.len());

        for quad in quads {
            let ink = self.ink(moved(quad.bounds, offset), quad.space.0);
            self.entries.push((Entry::Quad, ink));
        }
    }

    /// Draws every instance of resident revision `revision`, moved by `offset`, lane by lane in
    /// the order quads, shader quads, shadows, lines, mono, subpixel and colour sprites, marks.
    pub fn draw_resident(&mut self, revision: u64, offset: [f32; 2], chunk: &LoweredChunk) {
        let index = self.draw_chunk(revision, offset);
        let lanes = &mut self.frame.lanes;
        push_resident(&mut lanes.quads, index, chunk.quads.len());
        push_resident(&mut lanes.shader_quads, index, chunk.shader_quads.len());
        push_resident(&mut lanes.shadows, index, chunk.shadows.len());
        push_resident(&mut lanes.lines, index, chunk.lines.len());
        push_resident(&mut lanes.mono, index, chunk.mono.len());
        push_resident(&mut lanes.subpixel, index, chunk.subpixel.len());
        push_resident(&mut lanes.color, index, chunk.color.len());
        push_resident(&mut lanes.marks, index, chunk.marks.len());

        let quads = chunk
            .quads
            .iter()
            .map(|quad| (Entry::Quad, quad.bounds, quad.space.0));
        let shader_quads = chunk.shader_quads.iter().map(|quad| {
            let entry = Entry::ShaderQuad(quad.shader, quad.params);
            (entry, quad.bounds, quad.space.0)
        });
        let shadows = chunk
            .shadows
            .iter()
            .map(|shadow| (Entry::Shadow, shadow.bounds, shadow.shadow.space.0));
        let lines = chunk
            .lines
            .iter()
            .map(|line| (Entry::Line, line.bounds, line.space.0));
        let mono = chunk
            .mono
            .iter()
            .map(|sprite| (Entry::Mono(sprite.tile.page), sprite.bounds, sprite.space));
        let subpixel = chunk.subpixel.iter().map(|sprite| {
            let entry = Entry::Subpixel(sprite.tile.page);
            (entry, sprite.bounds, sprite.space)
        });
        let color = chunk
            .color
            .iter()
            .map(|sprite| (Entry::Color(sprite.tile.page), sprite.frame, sprite.space));
        let marks = chunk
            .marks
            .iter()
            .map(|marks| (Entry::Marks, marks.marks.bounds, marks.marks.space.0));
        let entries = quads
            .chain(shader_quads)
            .chain(shadows)
            .chain(lines)
            .chain(mono)
            .chain(subpixel)
            .chain(color)
            .chain(marks);
        for (entry, bounds, space) in entries {
            let ink = self.ink(moved(bounds, offset), space);
            self.entries.push((entry, ink));
        }
    }

    /// Adds resident revision `revision` to the chunks the frame draws, and returns its index.
    fn draw_chunk(&mut self, revision: u64, offset: [f32; 2]) -> u32 {
        let index = self.frame.drawn.len() as u32;
        self.frame.drawn.push(DrawnChunk { revision, offset });
        index
    }
}

/// Appends the first `count` instances of drawn chunk `chunk` to the instances `lane` draws.
fn push_resident<T>(lane: &mut Lane<T>, chunk: u32, count: usize) {
    let sources = (0..count as u32).map(|index| ItemSource::resident(chunk, index));
    lane.sources.extend(sources);
}

fn moved([x, y, w, h]: [f32; 4], offset: [f32; 2]) -> [f32; 4] {
    [x + offset[0], y + offset[1], w, h]
}
