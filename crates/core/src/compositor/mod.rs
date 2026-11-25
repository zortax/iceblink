//! The compositor type and its construction.

mod api;
mod caches;
mod clients;
mod outputs;
mod text;

use std::sync::Arc;

use iceblink::{ClientId, CompositorEvent, Counters};
use iceblink::{CompositorBudgets, FontRegistry};
use rustc_hash::FxHashMap;

use crate::client::Client;
use crate::commit::journal::Journal;
use crate::commit::validate::ChunkNets;
use crate::font::OwnFonts;
use crate::frame::FrameCheck;
use crate::frame::keys::PassDigests;
use crate::frame::walk::Scratch;
use crate::output::Outputs;
use crate::resident::Revisions;
use crate::scene::Scene;
use crate::shared_content::Content;
use crate::shift::Refusals;
use crate::tables::FrameTables;
use crate::work::Work;

/// The reference compositor: a retained store of clients' chunks, nodes and resources that
/// derives damage and builds frames for its outputs.
///
/// The compositor holds no device, so any renderer can draw its frames.
pub struct CompositorImpl {
    pub(crate) clients: FxHashMap<ClientId, Client>,
    pub(crate) next_client: u32,
    pub(crate) outputs: Outputs,
    pub(crate) next_output: u32,
    pub(crate) tables: FrameTables,
    pub(crate) revisions: Revisions,
    pub(crate) content: Content,
    pub(crate) fonts: Arc<dyn FontRegistry>,
    pub(crate) counters: Counters,
    pub(crate) events: Vec<CompositorEvent>,
    pub(crate) verification: bool,
    pub(crate) budgets: CompositorBudgets,
    /// The serial of the next frame built, which orders slot releases against frames.
    pub(crate) serial: u64,
    /// How many differences verification found between cached and recomputed state.
    pub(crate) mismatches: u64,
    /// What each pass key was issued for, recorded under verification.
    pub(crate) pass_digests: PassDigests,
    /// The scene an output without a root draws.
    pub(crate) empty: Scene,
    /// The buffers frame walks reuse.
    pub(crate) scratch: Scratch,
    /// The check frames built under verification must pass.
    pub(crate) frame_check: Option<FrameCheck>,
    /// How many frames failed the frame check.
    pub(crate) check_failures: u64,
    /// How many pairs of overlapping objects shared a draw order in frames built under
    /// verification.
    pub(crate) order_overlaps: u64,
    pub(crate) work: Work,
    /// The journal commits fill, kept empty between commits for its capacity.
    pub(crate) journal: Journal,
    /// The changed chunk keys commits fill, kept empty between commits for their capacity.
    pub(crate) chunk_nets: ChunkNets,
    pub(crate) refusals: Refusals,
    /// The `now_ns` of the first frame request, which the shader clock counts from.
    pub(crate) clock_start: Option<u64>,
    /// Whether a frame drew a glyph run as nothing for want of a glyph rasteriser.
    // The miss is logged once.
    pub(crate) glyphs_missed: bool,
}

impl CompositorImpl {
    /// Creates a compositor with no clients, no outputs and a font registry of its own.
    pub fn new() -> Self {
        let fonts: Arc<dyn FontRegistry> = Arc::new(OwnFonts::default());
        Self {
            clients: FxHashMap::default(),
            next_client: 1,
            outputs: Outputs::default(),
            next_output: 1,
            tables: FrameTables::default(),
            revisions: Revisions::default(),
            content: Content::new(Arc::clone(&fonts)),
            fonts,
            counters: Counters::default(),
            events: Vec::new(),
            verification: crate::invariant::enabled(),
            budgets: CompositorBudgets::default(),
            serial: 1,
            mismatches: 0,
            pass_digests: PassDigests::default(),
            empty: Scene::new(ClientId(0)),
            scratch: Scratch::default(),
            frame_check: None,
            check_failures: 0,
            order_overlaps: 0,
            work: Work::default(),
            journal: Journal::default(),
            chunk_nets: ChunkNets::default(),
            refusals: Refusals::default(),
            clock_start: None,
            glyphs_missed: false,
        }
    }
}

impl Default for CompositorImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for CompositorImpl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CompositorImpl")
            .field("clients", &self.clients.len())
            .field("outputs", &self.outputs.len())
            .finish_non_exhaustive()
    }
}
