//! The naive compositor and its implementation of the scene seam.

mod build;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use iceblink::smallvec::SmallVec;
use iceblink::{
    Built, ClientId, CommitError, CompositorEvent, Counters, Frame, FrameId, FrameOutcome,
    FrameRequest, HitResult, IdSpace, OutputId, ScrollGesture, ScrollRouting, Transaction,
};
use iceblink::{
    CommitReport, Compositor, CompositorBudgets, CompositorCapabilities, CompositorMemory,
    FontRegistry, GlyphRaster, OutputConfig,
};

use crate::naive::commit::{self, Context};
use crate::naive::content::Content;
use crate::naive::output::Output;
use crate::naive::registry::MemoryFontRegistry;
use crate::naive::scene::{Chunk, Client, Scene};

/// A compositor that implements the shared compositor contract in the simplest sound way.
///
/// It conforms to the soundness profile. Every frame that changed anything damages the whole
/// output, and every item is lowered again on every frame. It holds no resident chunks, emits no
/// shifts, opaque covers or pass keys, and implements none of the compositor-driven tier.
pub struct NaiveCompositor {
    scene: Scene,
    outputs: HashMap<OutputId, Output>,
    next_output: u32,
    next_client: u32,
    fonts: Arc<dyn FontRegistry>,
    raster: Option<Arc<dyn GlyphRaster>>,
    content: Content,
    counters: Counters,
    budgets: CompositorBudgets,
    /// The last image version handed out.
    versions: u64,
    /// The `now_ns` of the first `build_frame` call, which shader time counts from.
    started: Option<u64>,
    /// Frames built across every output, which orders pages by when they were last drawn.
    builds: u64,
    /// The smallest `max_texture_size` of every renderer a frame was built for, which bounds
    /// every page.
    page_limit: u32,
}

impl NaiveCompositor {
    /// Returns a compositor with no client, no output, a font registry of its own and no glyph
    /// rasteriser.
    pub fn new() -> Self {
        Self {
            scene: Scene::default(),
            outputs: HashMap::new(),
            next_output: 0,
            next_client: 0,
            fonts: Arc::new(MemoryFontRegistry::new()),
            raster: None,
            content: Content::default(),
            counters: Counters::default(),
            budgets: CompositorBudgets::default(),
            versions: 0,
            started: None,
            builds: 0,
            page_limit: u32::MAX,
        }
    }

    /// Marks every output as owing a frame.
    fn changed(&mut self) {
        for output in self.outputs.values_mut() {
            output.owes_frame = true;
        }
    }

    /// Forgets the route histories of the paths the scene no longer holds.
    fn forget_removed_paths(&mut self) {
        let scene = &self.scene;
        self.content.routes.keep(|id| scene.holds_path(id));
    }

    /// Drops every page and cached raster, and treats every renderer as one that holds nothing.
    ///
    /// Returns the bytes of the dropped pages.
    fn drop_caches(&mut self) -> u64 {
        let bytes = self.content.forget();
        for output in self.outputs.values_mut() {
            output.reset_resident = true;
            output.owes_frame = true;
        }
        bytes
    }

    /// Drops every page and cached raster after a glyph rasteriser or font registry swap, and
    /// damages every output when any chunk exists.
    fn swapped(&mut self) {
        // Without chunks the caches still go, or a later chunk would draw the old tiles.
        if self.has_chunks() {
            self.drop_caches();
        } else {
            self.content.forget();
        }
    }

    /// Returns the pages the last frame of any output drew from.
    fn pinned(&self) -> HashSet<u32> {
        self.outputs
            .values()
            .flat_map(|output| output.drawn_pages.iter().copied())
            .collect()
    }

    fn has_chunks(&self) -> bool {
        self.scene
            .clients
            .values()
            .any(|client| !client.chunks.is_empty())
    }
}

impl Default for NaiveCompositor {
    fn default() -> Self {
        Self::new()
    }
}

impl Compositor for NaiveCompositor {
    fn capabilities(&self) -> CompositorCapabilities {
        CompositorCapabilities::default()
    }

    fn register_client(&mut self) -> (ClientId, IdSpace) {
        let id = ClientId(self.next_client);
        self.next_client += 1;
        let ids = IdSpace::new(id);
        self.scene.clients.insert(id, Client::new(ids.clone()));
        (id, ids)
    }

    fn unregister_client(&mut self, client: ClientId) {
        if self.scene.clients.remove(&client).is_none() {
            return;
        }

        self.scene.roots.retain(|_, (owner, _)| *owner != client);
        self.forget_removed_paths();
        self.changed();
    }

    fn transaction(&mut self, client: ClientId) -> Transaction {
        let ids = match self.scene.clients.get(&client) {
            Some(held) => held.ids.clone(),
            // A transaction for an unknown client is rejected at commit.
            None => IdSpace::new(client),
        };
        Transaction::new(ids)
    }

    fn commit(&mut self, transaction: Transaction) -> Result<CommitReport, CommitError> {
        let mut parts = transaction.into_parts();
        let empty = parts.commands.is_empty();
        let outputs: HashSet<OutputId> = self.outputs.keys().copied().collect();
        let context = Context {
            outputs: &outputs,
            fonts: self.fonts.as_ref(),
            versions: &mut self.versions,
        };

        let committed = match commit::commit(&self.scene, &mut parts, context) {
            Ok(committed) => committed,
            Err(error) => {
                parts.ids.free(&parts.minted);
                return Err(error);
            }
        };

        self.scene.clients.insert(committed.client, committed.state);
        self.scene.roots = committed.roots;
        self.forget_removed_paths();
        parts.ids.free(&committed.removed);
        self.counters.commit = committed.counters;
        if !empty {
            self.changed();
        }

        Ok(CommitReport {
            animations: SmallVec::new(),
            counters: committed.counters,
        })
    }

    fn use_font_registry(&mut self, registry: Arc<dyn FontRegistry>) {
        self.fonts = registry;
        self.swapped();
    }

    fn font_registry(&self) -> Arc<dyn FontRegistry> {
        self.fonts.clone()
    }

    fn use_glyph_raster(&mut self, raster: Arc<dyn GlyphRaster>) {
        self.raster = Some(raster);
        self.swapped();
    }

    fn create_output(&mut self, config: OutputConfig) -> OutputId {
        let id = OutputId(self.next_output);
        self.next_output += 1;
        self.outputs.insert(id, Output::new(config));
        id
    }

    fn configure_output(&mut self, output: OutputId, config: OutputConfig) {
        if let Some(held) = self.outputs.get_mut(&output) {
            held.config = config;
            held.owes_frame = true;
        }
    }

    fn destroy_output(&mut self, output: OutputId) {
        self.outputs.remove(&output);
        self.scene.roots.remove(&output);
    }

    fn build_frame(
        &mut self,
        output: OutputId,
        request: &FrameRequest<'_>,
        into: &mut Frame,
    ) -> Built {
        self.build(output, request, into)
    }

    fn next_wake(&self, output: OutputId) -> Option<u64> {
        // Without the compositor-driven tier, only content that reads the clock and frame paths
        // waiting to take masks again change on their own.
        let reads_time = self
            .outputs
            .get(&output)
            .is_some_and(|state| state.reads_time);
        (reads_time || self.content.routes.waiting()).then_some(0)
    }

    fn frame_done(&mut self, output: OutputId, frame: FrameId, outcome: &FrameOutcome) {
        if let Some(held) = self.outputs.get_mut(&output) {
            held.done(frame, outcome);
        }
    }

    fn scroll_input(&mut self, _output: OutputId, _gesture: ScrollGesture) -> ScrollRouting {
        ScrollRouting::Unhandled
    }

    fn hit_test(&self, _output: OutputId, _point: [f32; 2]) -> SmallVec<[HitResult; 4]> {
        SmallVec::new()
    }

    fn drain_events(&mut self, _out: &mut Vec<CompositorEvent>) {}

    fn set_verification(&mut self, _on: bool) {}

    fn counters(&self) -> &Counters {
        &self.counters
    }

    fn memory(&self) -> CompositorMemory {
        let pinned = self.pinned();
        let pages = &self.content.pages;
        let pinned_bytes = pages
            .pages
            .iter()
            .filter(|(id, _)| pinned.contains(id))
            .map(|(_, page)| page.bytes())
            .sum();
        let clients = self.scene.clients.values();
        let image_bytes = clients
            .clone()
            .flat_map(|client| client.images.iter().map(|(_, _, image)| image.bytes()))
            .sum();
        let chunk_bytes = clients
            .map(|client| client.chunks.len() as u64 * size_of::<Chunk>() as u64)
            .sum();

        CompositorMemory {
            chunk_bytes,
            table_bytes: 0,
            atlas_bytes: pages.bytes(),
            atlas_pinned_bytes: pinned_bytes,
            image_bytes,
        }
    }

    fn set_budgets(&mut self, budgets: CompositorBudgets) {
        self.budgets = budgets;
    }

    fn budgets(&self) -> CompositorBudgets {
        self.budgets
    }

    fn trim(&mut self, to_bytes: u64) -> u64 {
        let pinned = self.pinned();
        self.content.evict(to_bytes, &pinned)
    }

    fn forget_caches(&mut self) -> u64 {
        self.drop_caches()
    }
}
