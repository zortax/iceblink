//! What a renderer holds for an output: tables, atlas pages, shaders and resident chunks.

mod page;

use std::collections::BTreeMap;
use std::sync::Arc;

use iceblink::{
    ChunkNote, ClipEntry, Frame, FramePaint, LoweredChunk, Matrix4, PageOp, ParamsBlock,
    ShaderDesc, ShaderOp,
};

pub use self::page::{Page, texel_bytes};

/// A copy of everything a renderer holds for an output, kept up to date by applying each frame's
/// work as a renderer does.
///
/// Operations on a page, shader slot or revision the mirror does not hold are skipped.
#[derive(Clone, Debug, Default)]
pub struct Mirror {
    /// World matrices, by frame space slot.
    pub spaces: Vec<Matrix4>,
    /// Clip chains, by frame clip slot.
    pub clips: Vec<ClipEntry>,
    /// Paints, by frame paint slot.
    pub paints: Vec<FramePaint>,
    /// Shader parameter blocks, by frame parameter slot.
    pub params: Vec<ParamsBlock>,
    /// Atlas pages, by page.
    pub pages: BTreeMap<u32, Page>,
    /// Shaders, by frame shader slot.
    pub shaders: BTreeMap<u32, Arc<ShaderDesc>>,
    /// Resident lowered chunks, by revision.
    pub resident: BTreeMap<u64, Arc<LoweredChunk>>,
}

impl Mirror {
    /// Returns a mirror of a renderer that holds nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies the frame's page and shader operations, then its chunk notes, then its table
    /// deltas.
    pub fn apply(&mut self, frame: &Frame) {
        for op in &frame.resources.pages {
            self.page_op(op);
        }
        for op in &frame.resources.shaders {
            self.shader_op(op);
        }

        for note in &frame.chunk_notes {
            self.note(note);
        }

        let tables = &frame.tables;
        tables.spaces.apply(&mut self.spaces);
        tables.clips.apply(&mut self.clips);
        tables.paints.apply(&mut self.paints);
        tables.params.apply(&mut self.params);
    }

    /// Forgets everything, as a renderer does when its device is lost.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn page_op(&mut self, op: &PageOp) {
        match op {
            PageOp::Create {
                page,
                format,
                size,
                levels,
            } => {
                self.pages
                    .insert(*page, Page::with_levels(*format, *size, *levels));
            }
            PageOp::Upload {
                page,
                level,
                rect,
                bytes,
                offset,
                stride,
            } => {
                if let Some(held) = self.pages.get_mut(page) {
                    held.upload_level(*level, *rect, bytes, *offset, *stride);
                }
            }
            PageOp::Destroy { page } => {
                self.pages.remove(page);
            }
        }
    }

    fn shader_op(&mut self, op: &ShaderOp) {
        match op {
            ShaderOp::Create { shader, desc } => {
                self.shaders.insert(*shader, desc.clone());
            }
            ShaderOp::Destroy { shader } => {
                self.shaders.remove(shader);
            }
        }
    }

    fn note(&mut self, note: &ChunkNote) {
        match note {
            ChunkNote::Inserted { revision, chunk } => {
                self.resident.insert(*revision, chunk.clone());
            }
            ChunkNote::Retired { revision } => {
                self.resident.remove(revision);
            }
            ChunkNote::ResetAll => self.resident.clear(),
        }
    }
}
