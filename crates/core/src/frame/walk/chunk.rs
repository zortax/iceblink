//! Emitting the items of one chunk that meet the damage.

use std::sync::Arc;

use iceblink::damage::DamageSet;
use iceblink::{
    ChunkKey, ChunkNote, DeviceRect, DrawnChunk, ItemKind, LoweredChunk, RecordedChunk,
};
use smallvec::SmallVec;

use crate::frame::emit;
use crate::frame::lower::{Lowering, View, ensure};
use crate::frame::walk::Walk;
use crate::frame::walk::instance::Placed;
use crate::frame::walk::{orders, paths};
use crate::scene::chunk::{LaneOrders, LoweredItem, LoweredPath, LoweringKey, Part};
use crate::scene::recount;

/// The arrays of a lowering and where the walk draws them.
struct Arrays {
    chunk: Arc<LoweredChunk>,
    parts: Arc<[Part]>,
    paths: Arc<[LoweredPath]>,
    /// The device translation of the frame paths placed under each client space.
    moves: SmallVec<[(u32, [f64; 2]); 1]>,
    orders: LaneOrders,
    revision: u64,
    /// The offset added to the lowered instances.
    delta: [f32; 2],
}

impl Walk<'_> {
    /// Emits the items of chunk `key` that meet the damage, drawn through the clip chain
    /// `context`.
    pub(super) fn chunk(&mut self, key: ChunkKey, context: u32) {
        let Some(record) = self.scene.chunks.get(&key) else {
            return;
        };
        if !self.meets(self.scene.device_inks(&record.derived.inks).union()) {
            self.counters.items_culled += record.recorded.ops.len() as u32;
            return;
        }

        let text = !record.recorded.glyph_runs.is_empty();
        let lowering = LoweringKey {
            custom_shaders: self.caps.custom_shaders,
            context,
            subpixel: self.text.subpixel.filter(|_| text),
            isolated: text && self.isolated > 0,
            scale_bits: if text { self.text.scale.to_bits() } else { 0 },
        };
        let mut inks = core::mem::take(&mut self.scratch.inks_of_items);
        inks.clear();
        inks.extend(self.scene.device_inks(&record.derived.inks).items());

        let scene = &mut *self.scene;
        let view = View {
            transforms: &scene.transforms,
            clips: &scene.clips,
            paints: &scene.paints,
            params: &scene.params,
            shaders: &scene.shaders,
        };
        let Some(record) = scene.chunks.get_mut(&key) else {
            return;
        };
        let offset = record.offset;
        if text && !self.content.rasterises() {
            self.counters.glyph_runs_without_raster +=
                runs_meeting(&record.recorded, &inks, &self.draw);
        }

        let mut parts = Lowering {
            tables: self.tables,
            revisions: self.revisions,
            content: self.content,
            work: self.work,
            client: self.client,
        };
        // Only a new lowering, or the glyph runs a lowering takes, changes the chunk's bytes.
        let before = record
            .derived
            .lowered
            .as_ref()
            .map(|lowered| lowered.revision);
        let took_runs = !record.derived.lowered_runs.is_empty();
        let lowered = ensure(view, key, record, lowering, &mut parts);
        let revision = lowered.revision;
        let recounted = took_runs || before != Some(revision);

        if let Some(stamp) = lowered.content.as_mut() {
            if self.caps.resident_chunks {
                if !stamp.resident {
                    self.content.cache.pin(&stamp.tiles);
                    stamp.resident = true;
                }
            } else {
                self.pins
                    .hold(revision, stamp.epoch, &stamp.tiles, self.content);
            }
        }

        let delta = [offset[0] - lowered.base[0], offset[1] - lowered.base[1]];
        let items = Arc::clone(&lowered.items);
        let arrays = Arrays {
            chunk: Arc::clone(&lowered.chunk),
            parts: Arc::clone(&lowered.parts),
            moves: if lowered.paths.is_empty() {
                SmallVec::new()
            } else {
                paths::path_moves(view, lowered, offset)
            },
            paths: Arc::clone(&lowered.paths),
            orders: orders::current(
                view,
                record,
                &self.tables.spaces,
                &mut self.scratch.orders,
                self.work,
            ),
            revision,
            delta,
        };
        if recounted {
            recount(&mut scene.chunk_bytes, record);
        }

        let mut drawn = 0;
        for (position, item) in items.iter().enumerate() {
            if *item == LoweredItem::Nothing {
                continue;
            }
            let ink = inks.get(position).copied().flatten();
            let Some(ink) = ink.filter(|ink| self.meets_rect(*ink)) else {
                self.counters.items_culled += 1;
                continue;
            };
            drawn += 1;
            self.item(*item, &arrays, (key, position), context, ink);
        }

        self.flush();
        if drawn > 0 {
            self.counters.chunks_drawn += 1;
        }
        self.scratch.inks_of_items = inks;
    }

    /// Emits the lowered item at `position` in the ops of chunk `key`, with device ink `ink`,
    /// drawn through the clip chain `context`.
    fn item(
        &mut self,
        item: LoweredItem,
        arrays: &Arrays,
        (key, position): (ChunkKey, usize),
        context: u32,
        ink: DeviceRect,
    ) {
        match item {
            LoweredItem::Instances { lane, start, len } => {
                let placed = Placed {
                    chunk: &arrays.chunk,
                    resident: self.resident(arrays.revision, arrays.delta, &arrays.chunk),
                    delta: arrays.delta,
                    ink,
                    alone: len == 1,
                    orders: &arrays.orders,
                };
                self.instances(lane, start, len, &placed);
            }
            LoweredItem::Parts { start, len } => {
                let resident = self.resident(arrays.revision, arrays.delta, &arrays.chunk);
                let range = start as usize..(start + len) as usize;
                for part in arrays.parts.get(range).unwrap_or(&[]) {
                    match *part {
                        Part::Instances { lane, start, len } => {
                            let placed = Placed {
                                chunk: &arrays.chunk,
                                resident,
                                delta: arrays.delta,
                                ink,
                                alone: false,
                                orders: &arrays.orders,
                            };
                            self.instances(lane, start, len, &placed);
                        }
                        Part::Path(index) => {
                            if let Some(path) = arrays.paths.get(index as usize) {
                                let by = arrays
                                    .moves
                                    .iter()
                                    .find(|(space, _)| *space == path.space)
                                    .map_or([0.0; 2], |&(_, by)| by);
                                self.flush();
                                emit::lowered_path(self, path, by, ink);
                            }
                        }
                    }
                }
            }
            LoweredItem::Single => {
                self.flush();
                emit::single(self, key, position, context, ink);
            }
            LoweredItem::Nothing => {}
        }
    }

    /// Returns the index of a revision in `Frame::drawn`, inserting it in the renderer first, when
    /// the renderer keeps chunks resident.
    fn resident(
        &mut self,
        revision: u64,
        offset: [f32; 2],
        chunk: &Arc<LoweredChunk>,
    ) -> Option<u32> {
        if !self.caps.resident_chunks {
            return None;
        }
        if let Some(&index) = self.scratch.drawn.get(&revision) {
            return Some(index);
        }

        if self.renderer.resident.insert(revision) {
            self.frame.chunk_notes.push(ChunkNote::Inserted {
                revision,
                chunk: Arc::clone(chunk),
            });
        }
        let index = self.frame.drawn.len() as u32;
        self.frame.drawn.push(DrawnChunk { revision, offset });
        self.scratch.drawn.insert(revision, index);
        Some(index)
    }
}

/// Counts the glyph runs of a chunk whose ink, from `inks`, meets `draw`.
fn runs_meeting(chunk: &RecordedChunk, inks: &[Option<DeviceRect>], draw: &DamageSet) -> u32 {
    chunk
        .ops
        .iter()
        .zip(inks)
        .filter(|(op, ink)| {
            op.kind == ItemKind::GLYPH_RUN
                && ink.is_some_and(|ink| draw.is_full() || draw.intersects(ink))
        })
        .count() as u32
}
