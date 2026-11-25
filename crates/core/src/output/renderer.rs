//! What an output's renderer holds: tables, resident chunks and shaders.

use rustc_hash::FxHashSet;

use crate::shared_content::pages::PageCursor;
use crate::tables::journal::TableVersion;

/// The state the compositor believes an output's renderer holds.
#[derive(Debug, Default)]
pub(crate) struct RendererState {
    /// Whether the renderer has drawn nothing for this output, or lost what it held.
    pub(crate) fresh: bool,
    /// The version of each frame table the renderer's copy matches: spaces, clips, paints and
    /// parameters.
    pub(crate) tables: [TableVersion; 4],
    /// The revisions the renderer holds resident.
    pub(crate) resident: FxHashSet<u64>,
    /// The position in the retirement log up to which retirements were considered.
    pub(crate) retired: u64,
    /// The frame shader slots the renderer was told to create.
    pub(crate) shaders: FxHashSet<u32>,
    /// Whether the kept target holds the previous frame, so a shift may build on it.
    pub(crate) kept: bool,
    /// Whether the renderer is to drop every resident chunk on the next frame.
    pub(crate) reset: bool,
    /// The atlas pages the renderer holds, and how far it read the page log.
    pub(crate) pages: PageCursor,
}

impl RendererState {
    /// The state of a renderer that has drawn nothing yet.
    pub(crate) fn fresh() -> Self {
        Self {
            fresh: true,
            ..Self::default()
        }
    }
}
