//! What a renderer holds for an output, as far as the frames seen so far tell it.

mod issued;
mod lookup;
mod registry;
mod table;

use std::collections::BTreeSet;
use std::sync::Arc;

use iceblink::{ClipEntry, FramePaint, LoweredChunk, Matrix4, PageFormat, ParamsBlock, Tables};

pub(crate) use self::issued::Issued;
pub(crate) use self::lookup::Lookup;
pub(crate) use self::registry::Registry;
pub(crate) use self::table::Table;

/// An atlas page a renderer holds.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Page {
    pub(crate) format: PageFormat,
    pub(crate) size: [u32; 2],
    pub(crate) levels: u32,
}

/// The tables, pages, shader slots and resident chunks a renderer holds for an output.
#[derive(Clone, Debug)]
pub(crate) struct Known {
    pub(crate) spaces: Table<Matrix4>,
    pub(crate) clips: Table<ClipEntry>,
    pub(crate) paints: Table<FramePaint>,
    pub(crate) params: Table<ParamsBlock>,
    pub(crate) pages: Registry<u32, Page>,
    pub(crate) shaders: Registry<u32, ()>,
    pub(crate) resident: Registry<u64, Arc<LoweredChunk>>,
    /// Revisions retired since the validator joined, which are never drawn again.
    pub(crate) retired: BTreeSet<u64>,
    /// The content each pass key was first issued for.
    pub(crate) keys: Issued,
}

impl Known {
    /// Returns the state of a renderer that has drawn nothing for the output.
    pub(crate) fn empty() -> Self {
        Self {
            spaces: Table::Complete(Vec::new()),
            clips: Table::Complete(Vec::new()),
            paints: Table::Complete(Vec::new()),
            params: Table::Complete(Vec::new()),
            pages: Registry::empty(),
            shaders: Registry::empty(),
            resident: Registry::empty(),
            retired: BTreeSet::new(),
            keys: Issued::default(),
        }
    }

    /// Returns the state of a renderer whose earlier frames were not seen.
    pub(crate) fn unknown() -> Self {
        Self {
            spaces: Table::Partial(Default::default()),
            clips: Table::Partial(Default::default()),
            paints: Table::Partial(Default::default()),
            params: Table::Partial(Default::default()),
            pages: Registry::unknown(),
            shaders: Registry::unknown(),
            resident: Registry::unknown(),
            retired: BTreeSet::new(),
            keys: Issued::default(),
        }
    }

    /// Applies a frame's table deltas.
    pub(crate) fn apply_tables(&mut self, tables: &Tables) {
        self.spaces.apply(&tables.spaces);
        self.clips.apply(&tables.clips);
        self.paints.apply(&tables.paints);
        self.params.apply(&tables.params);
    }
}
