//! The renderer's copies of the frame tables, kept by applying each frame's deltas, and the
//! spaces and clips resolved from them.

mod clip;
mod paint;
mod space;

use iceblink::{ClipEntry, FramePaint, Matrix4};

pub(crate) use self::clip::Clip;
pub(crate) use self::paint::Painter;
pub(crate) use self::space::Space;

/// The frame tables as the renderer holds them.
#[derive(Debug, Default)]
pub(crate) struct Tables {
    matrices: Vec<Matrix4>,
    entries: Vec<ClipEntry>,
    paints: Vec<FramePaint>,
    spaces: Vec<Space>,
    clips: Vec<Clip>,
}

impl Tables {
    /// Applies a frame's table deltas.
    ///
    /// Solid and side colours are quantised to eight bits per component. Spaces and clips are
    /// resolved again when the space or clip table changed.
    pub(crate) fn apply(&mut self, tables: &iceblink::Tables) {
        let spaces_changed = !tables.spaces.is_unchanged();
        let clips_changed = !tables.clips.is_unchanged();
        tables.spaces.apply(&mut self.matrices);
        tables.clips.apply(&mut self.entries);
        tables.paints.apply(&mut self.paints);
        if tables.paints.replace {
            self.paints.iter_mut().for_each(FramePaint::quantize);
        }
        for &(slot, _) in &tables.paints.changed {
            self.paints[slot as usize].quantize();
        }
        if spaces_changed {
            self.spaces.clear();
            self.spaces.extend(self.matrices.iter().map(Space::of));
        }
        if spaces_changed || clips_changed {
            self.clips.clear();
            let matrices = &self.matrices;
            self.clips.extend(
                self.entries
                    .iter()
                    .map(|entry| Clip::resolve(entry, matrices)),
            );
        }
    }

    /// Returns the resolved spaces, by frame space slot.
    pub(crate) fn spaces(&self) -> &[Space] {
        &self.spaces
    }

    /// Returns the resolved clips, by frame clip slot.
    pub(crate) fn clips(&self) -> &[Clip] {
        &self.clips
    }

    /// Returns the clip chains, by frame clip slot.
    pub(crate) fn entries(&self) -> &[ClipEntry] {
        &self.entries
    }

    /// Returns the world matrices, by frame space slot.
    pub(crate) fn matrices(&self) -> &[Matrix4] {
        &self.matrices
    }

    /// Returns the paints, by frame paint slot.
    pub(crate) fn paints(&self) -> &[FramePaint] {
        &self.paints
    }
}
