//! The content an output draws that needs extra damage: clock-reading shaders and composites
//! that read outside what they write.

use iceblink::{ChunkKey, DeviceRect, Filter, ShaderReads, SpaceRef, read_extent};

use crate::derive::order::exact_ink;
use crate::derive::spread::Spread;
use crate::geometry::bounds::{DeviceBox, map};
use crate::scene::Scene;

/// Returns the ink of every chunk and group drawn under `root` that draws with a clock-reading
/// shader, grown by the spread of the groups around it.
pub(crate) fn timed(scene: &Scene, root: u32) -> Vec<DeviceRect> {
    let mut inks: Vec<DeviceRect> = scene
        .timed
        .iter()
        .filter(|key| chunk_under(scene, **key, root))
        .filter_map(|key| {
            let record = scene.chunks.get(key)?;
            let ink = scene.device_inks(&record.derived.inks).union()?;
            let parent = scene.placements.first(key)?;
            Some(Spread::inside(scene, parent).grow(ink))
        })
        .collect();

    let reads_time = |shader: u32| {
        scene
            .shaders
            .at(shader)
            .is_some_and(|record| record.desc.reads.contains(ShaderReads::TIME))
    };
    for &effect in &scene.shaded_effects {
        let Some(record) = scene.effects.at(effect) else {
            continue;
        };
        let timed = record.node.filters.iter().any(
            |filter| matches!(filter, Filter::Shader { shader, .. } if reads_time(shader.slot())),
        );
        if !timed {
            continue;
        }
        for &node in &record.orders {
            if node_under(scene, node, root) {
                let spread = Spread::above(scene, node);
                inks.extend(exact_ink(scene, node).map(|ink| spread.grow(ink)));
            }
        }
    }
    inks
}

/// Returns the read extent of every reading composite drawn under `root`, cut to `output`.
pub(crate) fn reading(scene: &Scene, root: u32, output: DeviceRect) -> Vec<DeviceRect> {
    let mut extents = Vec::new();
    for &slot in &scene.reading_groups {
        if !node_under(scene, slot, root) {
            continue;
        }
        if let Some(ink) = exact_ink(scene, slot) {
            extents.extend(ink.intersection(output));
        }
    }

    for &key in &scene.reading {
        if !chunk_under(scene, key, root) {
            continue;
        }
        let Some(record) = scene.chunks.get(&key) else {
            continue;
        };
        let chunk = &record.recorded;
        for backdrop in &chunk.backdrops {
            let Some(chain) = chunk.filters.get(backdrop.filters as usize) else {
                continue;
            };
            let slot = if backdrop.space == SpaceRef::INHERIT {
                chunk.props.space.slot()
            } else {
                backdrop.space.0
            };
            let Some(space) = scene.transforms.at(slot) else {
                continue;
            };

            let mut bounds = backdrop.bounds;
            bounds[0] += record.offset[0];
            bounds[1] += record.offset[1];
            let read = read_extent(map(&space.world, bounds).rect(), chain);
            extents.extend(DeviceBox::of(read).pixels(output));
        }
    }
    extents
}

/// Reports whether the chunk `key` is drawn under the order node `root`.
pub(crate) fn chunk_under(scene: &Scene, key: ChunkKey, root: u32) -> bool {
    scene
        .placements
        .first(&key)
        .is_some_and(|parent| node_under(scene, parent, root))
}

/// Reports whether the order node at `slot` is `root` or lies in its subtree.
pub(crate) fn node_under(scene: &Scene, slot: u32, root: u32) -> bool {
    let mut current = slot;
    for _ in 0..=scene.orders.span() {
        if current == root {
            return true;
        }
        match scene.node_placements.first(current) {
            Some(parent) => current = parent,
            None => return false,
        }
    }
    false
}
