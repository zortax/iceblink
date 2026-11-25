//! The walk over an output's paint order, from last to first.

use iceblink::{
    ChunkFlags, ChunkKey, ClientId, ClipRef, DeviceRect, HitResult, ItemKind, ItemOp, OrderEntry,
    RecordedChunk, SpaceRef,
};
use smallvec::SmallVec;

use crate::content::ImageStore;
use crate::hit::clip::Clips;
use crate::hit::plane::unproject;
use crate::hit::shape::contains;
use crate::scene::Scene;
use crate::scene::chunk::ChunkRecord;

/// One item under the point.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Hit {
    pub(crate) result: HitResult,
    /// The slot of the transform node the item is drawn in.
    pub(crate) space: u32,
}

/// A hit test in progress.
pub(crate) struct Walk<'a> {
    clips: Clips<'a>,
    client: ClientId,
    /// The clip nodes of the effects above the order node being visited.
    effect_clips: SmallVec<[u32; 4]>,
    found: &'a mut dyn FnMut(Hit) -> bool,
    done: bool,
}

impl<'a> Walk<'a> {
    pub(crate) fn new(
        scene: &'a Scene,
        images: &'a ImageStore,
        client: ClientId,
        point: [f32; 2],
        found: &'a mut dyn FnMut(Hit) -> bool,
    ) -> Self {
        Self {
            clips: Clips {
                scene,
                images,
                point,
            },
            client,
            effect_clips: SmallVec::new(),
            found,
            done: false,
        }
    }

    /// Visits the subtree of the order node `slot`.
    pub(crate) fn run(&mut self, slot: u32) {
        self.node(slot, 0);
    }

    fn scene(&self) -> &'a Scene {
        self.clips.scene
    }

    /// Visits the subtree of the order node `slot`, from its last entry to its first.
    fn node(&mut self, slot: u32, depth: usize) {
        let scene = self.scene();
        let Some(record) = scene.orders.at(slot) else {
            return;
        };
        if depth > scene.orders.span() {
            return;
        }
        if !record.dirty
            && let Some(ink) = scene.node_ink(&record.ink)
            && !self.meets(ink)
        {
            return;
        }

        let clip = record
            .node
            .effect
            .and_then(|effect| scene.effects.at(effect.slot()))
            .and_then(|effect| effect.node.clip);
        if let Some(clip) = clip {
            self.effect_clips.push(clip.slot());
        }

        let rows = record.rows.as_ref().map(|rows| {
            let carry = record.ink.carry();
            let travel = carry.map_or(0, |carry| scene.travel(carry.scroll)[1]);
            let top = (self.clips.point[1].floor() as i32).saturating_sub(travel);
            rows.meeting(top, top.saturating_add(1))
        });
        let steps = rows.clone().unwrap_or(0..record.entries.len());

        for step in steps.rev() {
            let index = match (&rows, &record.rows) {
                (Some(_), Some(held)) => held.entry(step),
                _ => Some(step),
            };
            let Some(entry) = index.and_then(|index| record.entries.get(index)) else {
                continue;
            };

            match *entry {
                OrderEntry::Chunk(key) => self.chunk(key),
                OrderEntry::Node(id) => self.node(id.slot(), depth + 1),
            }
            if self.done {
                break;
            }
        }

        if clip.is_some() {
            self.effect_clips.pop();
        }
    }

    /// Visits the items of chunk `key`, from last to first.
    fn chunk(&mut self, key: ChunkKey) {
        let scene = self.scene();
        let Some(record) = scene.chunks.get(&key) else {
            return;
        };
        let recorded = &record.recorded;
        if recorded.props.flags.contains(ChunkFlags::HIT_TRANSPARENT) {
            return;
        }
        let inks = scene.device_inks(&record.derived.inks);
        // Stored inks bound the items only while there is one per item.
        let inked = inks.len() == recorded.ops.len();
        if inked && !inks.union().is_some_and(|union| self.meets(union)) {
            return;
        }

        for position in (0..recorded.ops.len()).rev() {
            if inked && !inks.item(position).is_some_and(|ink| self.meets(ink)) {
                continue;
            }
            if let Some(hit) = self.item(key, record, position)
                && !(self.found)(hit)
            {
                self.done = true;
                return;
            }
        }
    }

    /// Tests the item at `position` of a chunk.
    fn item(&self, key: ChunkKey, record: &ChunkRecord, position: usize) -> Option<Hit> {
        let scene = self.scene();
        let recorded = &record.recorded;
        let op = recorded.ops[position];
        let (space, clip) = refs(recorded, op)?;
        let space = if space == SpaceRef::INHERIT {
            recorded.props.space.slot()
        } else {
            space.0
        };
        let world = scene.transforms.at(space)?.world;
        let placed = unproject(&world, self.clips.point)?;
        let local = [placed[0] - record.offset[0], placed[1] - record.offset[1]];

        if !contains(recorded, op, &record.derived.runs, local) {
            return None;
        }
        if !self.clips.item(recorded, record.offset, clip) {
            return None;
        }
        if !self
            .effect_clips
            .iter()
            .all(|&slot| self.clips.node_chain(slot))
        {
            return None;
        }
        Some(Hit {
            result: HitResult {
                client: self.client,
                chunk: key,
                item: position as u32,
                local,
            },
            space,
        })
    }

    /// Reports whether the device rectangle `rect`, edges included, holds the point.
    fn meets(&self, rect: DeviceRect) -> bool {
        let [x, y] = self.clips.point;
        let (x0, y0) = (rect.origin.x as f32, rect.origin.y as f32);
        let (x1, y1) = (x0 + rect.size.width as f32, y0 + rect.size.height as f32);
        x0 <= x && x <= x1 && y0 <= y && y <= y1
    }
}

/// Returns the space and clip references of an item.
pub(crate) fn refs(chunk: &RecordedChunk, op: ItemOp) -> Option<(SpaceRef, ClipRef)> {
    let index = op.index as usize;
    match op.kind {
        ItemKind::QUAD => chunk.quads.get(index).map(|i| (i.space, i.clip)),
        ItemKind::SHADER_QUAD => chunk.shader_quads.get(index).map(|i| (i.space, i.clip)),
        ItemKind::SHADOW => chunk.shadows.get(index).map(|i| (i.space, i.clip)),
        ItemKind::LINE => chunk.lines.get(index).map(|i| (i.space, i.clip)),
        ItemKind::GLYPH_RUN => chunk.glyph_runs.get(index).map(|i| (i.space, i.clip)),
        ItemKind::IMAGE => chunk.images.get(index).map(|i| (i.space, i.clip)),
        ItemKind::MARKS => chunk.marks.get(index).map(|i| (i.space, i.clip)),
        ItemKind::PATH => chunk.paths.get(index).map(|i| (i.space, i.clip)),
        ItemKind::EXTERNAL => chunk.externals.get(index).map(|i| (i.space, i.clip)),
        ItemKind::BACKDROP => chunk.backdrops.get(index).map(|i| (i.space, i.clip)),
        _ => None,
    }
}
