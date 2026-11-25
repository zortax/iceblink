//! Whether a chunk draws with a clock-reading shader or reads around a backdrop.

use iceblink::{ChunkKey, Filter, ShaderReads, read_extent};

use crate::scene::Scene;

/// Reports whether a chunk draws with a shader that reads the clock.
pub(super) fn reads_time(scene: &Scene, key: ChunkKey) -> bool {
    let Some(record) = scene.chunks.get(&key) else {
        return false;
    };
    let timed = |slot: u32| {
        scene
            .shaders
            .at(slot)
            .is_some_and(|shader| shader.desc.reads.contains(ShaderReads::TIME))
    };
    let quads = record
        .recorded
        .shader_quads
        .iter()
        .any(|quad| timed(quad.shader));
    let filters = record
        .recorded
        .filters
        .iter()
        .flat_map(|chain| chain.iter())
        .any(|filter| matches!(filter, Filter::Shader { shader, .. } if timed(shader.slot())));
    quads || filters
}

/// Reports whether a chunk holds a backdrop whose filters read outside what it writes.
pub(super) fn reads_around(scene: &Scene, key: ChunkKey) -> bool {
    let Some(record) = scene.chunks.get(&key) else {
        return false;
    };
    record.recorded.backdrops.iter().any(|backdrop| {
        record
            .recorded
            .filters
            .get(backdrop.filters as usize)
            .is_some_and(|chain| read_extent([0.0, 0.0, 1.0, 1.0], chain) != [0.0, 0.0, 1.0, 1.0])
    })
}
