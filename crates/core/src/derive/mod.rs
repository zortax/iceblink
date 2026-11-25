//! Bringing derived state up to date after a successful commit, and deriving its damage.
//!
//! The journal's old and new values decide what changed. Reverse indices, frame slots, world
//! matrices, clip bounds, item inks and order inks are updated for those objects only, and the
//! damage of each change goes to the outputs that draw it.
//!
//! A whole-pixel scroll adds to the travel of its scroll node, the whole pixels the node's offset
//! changes have moved the content it carries. That content is moved by the travel, so its inks are
//! not computed again.

pub(crate) mod carry;
pub(crate) mod chunks;
mod clips;
mod diff;
mod fresh;
pub(crate) mod inks;
mod links;
mod motion;
pub(crate) mod order;
mod resources;
pub(crate) mod route;
pub(crate) mod runs;
pub(crate) mod scroll;
pub(crate) mod spread;
mod world;

use iceblink::{CommitCounters, CompositorEvent, OrderEntry, RecordedChunk};

use crate::commit::journal::Journal;
use crate::commit::validate::ChunkNet;
use crate::content::keeps_pixels;
use crate::output::Outputs;
use crate::resident::Revisions;
use crate::scene::Scene;
use crate::shared_content::Content;
use crate::shared_content::prefetch::{RunOutput, prefetch};
use crate::tables::FrameTables;
use crate::work::Work;

/// What deriving one successful commit works on.
pub(crate) struct Derive<'a> {
    pub(crate) scene: &'a mut Scene,
    pub(crate) outputs: &'a mut Outputs,
    pub(crate) tables: &'a mut FrameTables,
    pub(crate) revisions: &'a mut Revisions,
    pub(crate) content: &'a mut Content,
    /// Recorded chunks whose buffers a later transaction can reuse.
    pub(crate) spare: &'a mut Vec<RecordedChunk>,
    pub(crate) events: &'a mut Vec<CompositorEvent>,
    pub(crate) counters: &'a mut CommitCounters,
    /// How many re-recorded chunks damaged pixels outside their damage hint.
    pub(crate) hint_violations: &'a mut u64,
    /// How many differences verification found between cached and recomputed state.
    pub(crate) mismatches: &'a mut u64,
    /// The build serial frame slots released now are quarantined at.
    pub(crate) serial: u64,
    /// Whether shortcuts and damage hints are checked against a full computation.
    pub(crate) verification: bool,
    /// What the output drawing the client allows its glyph runs.
    pub(crate) text: RunOutput,
    pub(crate) work: &'a mut Work,
}

impl Derive<'_> {
    /// Updates every derived structure for the changes in `journal`, and adds their damage to
    /// the outputs that draw them.
    pub(crate) fn run(&mut self, journal: &mut Journal, nets: &[ChunkNet], entries: &[OrderEntry]) {
        let routes = route::Routes::of(self.scene, self.outputs);

        // Damage measured on the state before the commit, while the caches still hold it.
        // A pass whose inputs the commit left empty is skipped, so its code is not run.
        if !(journal.orders.is_empty() && journal.effects.is_empty() && journal.chunks.is_empty()) {
            order::removed_damage(self, journal, &routes);
        }

        let resources = !journal.images.is_empty() || !journal.deferred.is_empty();
        if resources {
            resources::create(self, journal);
        }

        let mut moved = links::nodes(self, journal);
        let replaced = links::replaced(self.scene, journal);
        if !nets.is_empty() {
            links::chunks(self, nets, &replaced);
        }
        // A node that comes to rest draws its users again, as a write of its value would.
        moved.transforms.extend(
            journal
                .rested
                .iter()
                .map(|&slot| (slot, links::Cause::Write)),
        );

        let mut world = world::WorldChanges::default();
        world::refresh(self.scene, self.tables, &moved.transforms, &mut world);
        motion::start(self.scene, journal, &world);
        if !journal.rested.is_empty()
            || world
                .moved
                .values()
                .any(|moved| !keeps_pixels(&moved.before, &moved.after))
        {
            self.content.placements += 1;
        }
        let clips = clips::refresh(self, &moved.clips, &world);
        self.prefetch_fresh(journal, nets);

        let clipped = if clips.coverage.is_empty() {
            Vec::new()
        } else {
            order::clipped_groups(self, &clips, &routes)
        };
        let travels = scroll::Travels::advance(self.scene, &moved.transforms, &world);
        let mut moves = scroll::Moves::default();
        let changes = chunks::Changes {
            nets,
            world: &world,
            clips: &clips,
            travels: &travels,
            replaced: &replaced,
        };
        let mut touched = chunks::Touched::default();
        chunks::refresh(self, journal, &changes, &routes, &mut moves, &mut touched);
        if resources {
            resources::apply(self, journal, &routes);
        }

        order::refresh(self, journal, &touched, &routes, entries, &clipped);
        if !journal.roots.is_empty() {
            order::roots(self, journal);
        }

        scroll::carried(self, &mut moves, &world, &travels, &routes);
        scroll::keep_late(self, &moves);
        let scene = &*self.scene;
        if !scene.tier.origins.survives(scene, journal, nets, entries) {
            self.scene.tier.origins.forget();
        }

        if !journal.replaced.is_empty() {
            chunks::forget_removed_paths(self, journal);
            chunks::recycle(self, journal);
        }
    }

    /// Rasterises the glyphs of the chunks whose glyph runs can draw glyphs they did not draw
    /// before the commit.
    fn prefetch_fresh(&mut self, journal: &Journal, nets: &[ChunkNet]) {
        if nets.is_empty() {
            return;
        }

        let fresh = fresh::fresh_runs(self.scene, journal, nets);
        self.work.glyph_chunks_prefetched += fresh
            .iter()
            .filter(|key| {
                self.scene
                    .chunks
                    .get(key)
                    .is_some_and(|record| !record.recorded.glyph_runs.is_empty())
            })
            .count() as u64;
        if !fresh.is_empty() {
            prefetch(self.content, self.scene, &fresh, self.text);
        }
    }
}
