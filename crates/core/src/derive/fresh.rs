//! The chunks whose glyph runs can draw glyphs after a commit that they did not draw before.

use iceblink::{ChunkKey, SpaceRef};
use smallvec::SmallVec;

use crate::commit::journal::{ChunkChange, Journal};
use crate::commit::validate::ChunkNet;
use crate::scene::Scene;
use crate::small::SmallMap;

/// The most changed chunks whose offsets one commit tracks without allocating.
const INLINE: usize = 16;

/// Returns the chunks of `nets` that hold a chunk after the commit, except those whose only
/// change is an offset that moves every glyph run by whole device pixels.
pub(crate) fn fresh_runs(
    scene: &Scene,
    journal: &Journal,
    nets: &[ChunkNet],
) -> SmallVec<[ChunkKey; INLINE]> {
    // The offset before the commit, for the chunks the commit only moved.
    let mut moved: SmallMap<ChunkKey, Option<[f32; 2]>, INLINE> = SmallMap::default();
    for change in &journal.chunks {
        match change {
            ChunkChange::Offset { key, before } => {
                moved.get_or_insert_with(*key, || Some(*before));
            }
            ChunkChange::Replaced { key, .. } | ChunkChange::Props { key, .. } => {
                moved.insert(*key, None);
            }
        }
    }

    nets.iter()
        .filter(|net| net.after.is_some())
        .map(|net| net.key)
        .filter(|key| match moved.get(key) {
            Some(Some(before)) => !whole_pixels(scene, *key, *before),
            _ => true,
        })
        .collect()
}

/// Reports whether moving chunk `key` from offset `before` to its offset moves every glyph run
/// by whole device pixels.
fn whole_pixels(scene: &Scene, key: ChunkKey, before: [f32; 2]) -> bool {
    let Some(record) = scene.chunks.get(&key) else {
        return false;
    };
    let chunk = &record.recorded;
    let [dx, dy] = [record.offset[0] - before[0], record.offset[1] - before[1]];
    chunk.glyph_runs.iter().all(|run| {
        let space = if run.space == SpaceRef::INHERIT {
            chunk.props.space.slot()
        } else {
            run.space.0
        };
        let Some(affine) = scene
            .transforms
            .at(space)
            .and_then(|space| space.world.to_affine2())
        else {
            return false;
        };
        let x = affine.a * dx + affine.c * dy;
        let y = affine.b * dx + affine.d * dy;
        x.is_finite() && y.is_finite() && x == x.round() && y == y.round()
    })
}
