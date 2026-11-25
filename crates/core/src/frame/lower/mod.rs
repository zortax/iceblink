//! Lowering a chunk's items into frame kinds, with frame slots in place of client slots.

mod build;
mod bytes;
pub(crate) mod content;
mod kinds;
mod slots;
mod stamp;

use iceblink::{ChunkKey, ClientId, ItemKind};

use crate::content::PathOwner;
use crate::resident::Revisions;
use crate::scene::chunk::{ChunkRecord, Lowered, LoweredItem, LoweringKey, RunLowering};
use crate::shared_content::Content;
use crate::tables::FrameTables;
use crate::work::Work;

pub(crate) use self::bytes::{lowered_bytes, recorded_bytes};
pub(crate) use self::slots::{Slots, View};

use self::build::Build;

/// The state a lowering reads and updates besides the chunk.
pub(crate) struct Lowering<'a> {
    pub(crate) tables: &'a mut FrameTables,
    pub(crate) revisions: &'a mut Revisions,
    pub(crate) content: &'a mut Content,
    pub(crate) work: &'a mut Work,
    /// The client the chunk belongs to.
    pub(crate) client: ClientId,
}

/// Returns the lowering of the chunk of `record` for `key`.
///
/// The chunk is lowered again, and its old lowering retired, when the current lowering was made
/// for another key or its atlas content no longer holds.
pub(crate) fn ensure<'a>(
    scene: View<'_>,
    chunk: ChunkKey,
    record: &'a mut ChunkRecord,
    key: LoweringKey,
    parts: &mut Lowering<'_>,
) -> &'a mut Lowered {
    let offset = record.offset;
    let stale = record.derived.lowered.as_mut().is_none_or(|lowered| {
        lowered.key != key || stamp::stale(lowered, scene, offset, parts.content, parts.work)
    });

    // Runs lowered at commit serve this lowering only.
    let mut runs = core::mem::take(&mut record.derived.lowered_runs);
    if stale {
        if let Some(old) = record.derived.lowered.take() {
            retire(old, parts.revisions, parts.content, parts.tables);
        }

        let slots = Slots {
            scene,
            chunk: &record.recorded,
            locals: &record.derived.local_frames,
            context: key.context,
        };
        // Content placed on device pixels is lowered at the chunk's offset.
        let recorded = &record.recorded;
        let placed = !recorded.glyph_runs.is_empty()
            || !recorded.paths.is_empty()
            || !recorded.marks.is_empty();
        let base = if placed { record.offset } else { [0.0; 2] };

        let mark = parts.tables.combined.mark();
        let lowered = lower(&slots, parts, (chunk, key), base, &mut runs);
        parts.work.chunks_lowered += 1;
        record.derived.lowered = Some(Lowered {
            revision: parts.revisions.mint(),
            combined: parts.tables.combined.hold_taken(mark, key.context),
            ..lowered
        });
    }

    record
        .derived
        .lowered
        .as_mut()
        .expect("the lowering was just made")
}

/// Retires a lowering that will not be drawn again, and lets go of its pin of the renderers that
/// keep it resident and of its combined clip chains.
pub(crate) fn retire(
    mut lowered: Lowered,
    revisions: &mut Revisions,
    content: &mut Content,
    tables: &mut FrameTables,
) {
    revisions.retire(lowered.revision);
    tables.combined.let_go(&lowered.combined);
    unpin_resident(&mut lowered, content);
}

/// Releases the pin a lowering's tiles hold for the renderers that keep it resident.
pub(crate) fn unpin_resident(lowered: &mut Lowered, content: &mut Content) {
    if let Some(stamp) = lowered.content.as_mut().filter(|stamp| stamp.resident) {
        content.unpin(stamp.epoch, &stamp.tiles);
        stamp.resident = false;
    }
}

/// Lowers every item of a chunk at offset `base`.
///
/// A glyph run takes its lowering from `runs` when it was lowered with the same inputs.
fn lower(
    slots: &Slots<'_>,
    parts: &mut Lowering<'_>,
    (owner, key): (ChunkKey, LoweringKey),
    base: [f32; 2],
    runs: &mut [Option<RunLowering>],
) -> Lowered {
    let chunk = slots.chunk;
    let mut build = Build::new(base, chunk.ops.len());

    for (position, op) in chunk.ops.iter().enumerate() {
        let index = op.index as usize;
        let item = match op.kind {
            ItemKind::QUAD => chunk.quads.get(index).map_or(LoweredItem::Nothing, |item| {
                kinds::quad(slots, parts.tables, &mut build, *item)
            }),
            ItemKind::SHADER_QUAD => chunk
                .shader_quads
                .get(index)
                .map_or(LoweredItem::Nothing, |item| {
                    kinds::shader_quad(slots, parts.tables, key, &mut build, *item)
                }),
            ItemKind::SHADOW => chunk
                .shadows
                .get(index)
                .map_or(LoweredItem::Nothing, |item| {
                    kinds::shadow(slots, parts.tables, &mut build, *item)
                }),
            ItemKind::LINE => chunk.lines.get(index).map_or(LoweredItem::Nothing, |item| {
                kinds::line(slots, parts.tables, &mut build, *item)
            }),
            ItemKind::GLYPH_RUN => {
                chunk
                    .glyph_runs
                    .get(index)
                    .map_or(LoweredItem::Nothing, |run| {
                        let earlier = runs.get_mut(index).and_then(Option::take);
                        content::glyph_run(slots, parts, key, &mut build, (run, earlier))
                    })
            }
            ItemKind::IMAGE => chunk
                .images
                .get(index)
                .map_or(LoweredItem::Nothing, |image| {
                    content::image(slots, parts, &mut build, image)
                }),
            ItemKind::MARKS => chunk
                .marks
                .get(index)
                .map_or(LoweredItem::Nothing, |marks| {
                    content::marks(slots, parts, &mut build, marks)
                }),
            ItemKind::PATH => chunk.paths.get(index).map_or(LoweredItem::Nothing, |item| {
                let owner = PathOwner {
                    client: parts.client,
                    chunk: owner,
                    position: position as u32,
                };
                content::path(slots, parts, &mut build, item, owner)
            }),
            ItemKind::EXTERNAL | ItemKind::BACKDROP => LoweredItem::Single,
            _ => LoweredItem::Nothing,
        };
        build.items.push(item);
    }

    build.finish(key, parts.content)
}
