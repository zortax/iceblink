//! The shared tables instances index, sent as deltas against the renderer's copy.

mod clip;
mod paint;

use crate::geometry::Matrix4;
use crate::paint::ParamsBlock;

pub use self::clip::{ClipEntry, ClipLinkEntry, FrameClipShape};
pub use self::paint::{FrameGradient, FramePaint, FrameStop, StopEncoding};

/// The frame tables, each a delta against the renderer's copy as of the previous frame of the
/// same output.
///
/// The first frame a renderer draws for an output replaces every table.
#[derive(Clone, Debug, Default)]
pub struct Tables {
    /// World matrices, from local space to device space.
    pub spaces: TableDelta<Matrix4>,
    /// Clip chains.
    pub clips: TableDelta<ClipEntry>,
    /// Paints.
    pub paints: TableDelta<FramePaint>,
    /// Shader parameter blocks.
    pub params: TableDelta<ParamsBlock>,
}

impl Tables {
    /// Empties every delta, keeping the allocations.
    pub fn clear(&mut self) {
        self.spaces.clear();
        self.clips.clear();
        self.paints.clear();
        self.params.clear();
    }
}

/// A change to a table.
///
/// A delta with `replace` unset and no changed slot leaves the table unchanged.
#[derive(Clone, Debug, PartialEq)]
pub struct TableDelta<T> {
    /// Whether `all` replaces the whole table.
    pub replace: bool,
    /// The whole table, read only when `replace` is set.
    pub all: Vec<T>,
    /// The slots that changed, with their new values, applied after `all`.
    ///
    /// The listed slots are replaced and the rest are kept. A slot past the end of the table
    /// grows it, and the slots in between are unspecified and unreferenced.
    pub changed: Vec<(u32, T)>,
}

impl<T> Default for TableDelta<T> {
    fn default() -> Self {
        Self {
            replace: false,
            all: Vec::new(),
            changed: Vec::new(),
        }
    }
}

impl<T> TableDelta<T> {
    /// Reports whether the delta leaves the table unchanged.
    pub fn is_unchanged(&self) -> bool {
        !self.replace && self.changed.is_empty()
    }

    /// Empties the delta, keeping the allocations.
    pub fn clear(&mut self) {
        self.replace = false;
        self.all.clear();
        self.changed.clear();
    }
}

impl<T: Clone> TableDelta<T> {
    /// Applies the delta to `table`.
    pub fn apply(&self, table: &mut Vec<T>) {
        if self.replace {
            table.clone_from(&self.all);
        }

        for (slot, value) in &self.changed {
            let slot = *slot as usize;
            if slot >= table.len() {
                table.resize(slot + 1, value.clone());
            }
            table[slot] = value.clone();
        }
    }
}
