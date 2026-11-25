//! Opaque quads that back a shift's region.

use iceblink::{ChunkKey, ClipRef, DeviceRect, ItemKind, RecordedChunk, SpaceRef};

use crate::geometry::admits::{axis_aligned, holds, square};
use crate::geometry::bounds::{map, of_pixels};
use crate::geometry::clip::{item_bounds, local_bounds};
use crate::scene::Scene;
use crate::shared_content::prefetch::opaque_ref;

/// Reports whether the item at `position` of chunk `key` is a quad that paints one opaque solid
/// colour over all of `region`.
pub(super) fn covers(scene: &Scene, key: ChunkKey, position: usize, region: DeviceRect) -> bool {
    let Some(record) = scene.chunks.get(&key) else {
        return false;
    };
    let chunk = &record.recorded;
    let Some(op) = chunk.ops.get(position) else {
        return false;
    };
    if op.kind != ItemKind::QUAD {
        return false;
    }
    let Some(quad) = chunk.quads.get(op.index as usize) else {
        return false;
    };

    let opaque = |paint| opaque_ref(&scene.paints, paint);
    let borderless = quad.border.iter().all(|&width| width <= 0.0);
    let border = borderless || (opaque(quad.stroke) && quad.style == iceblink::BorderStyle::SOLID);
    let square = quad.radii.iter().all(|&radius| radius <= 0.0);
    if !(opaque(quad.fill) && border && square) {
        return false;
    }

    let space = if quad.space == SpaceRef::INHERIT {
        chunk.props.space.slot()
    } else {
        quad.space.0
    };
    let Some(world) = scene.transforms.at(space).map(|record| record.world) else {
        return false;
    };
    if !axis_aligned(&world) || !hard_chain(scene, chunk, quad.clip) {
        return false;
    }

    let mut bounds = quad.bounds;
    bounds[0] += record.offset[0];
    bounds[1] += record.offset[1];

    let mut locals = Vec::new();
    local_bounds(scene, chunk, record.offset, &mut locals);
    let clip = item_bounds(scene, chunk, &locals, quad.clip);
    let painted = map(&world, bounds).intersect(clip);
    holds(painted, of_pixels(region))
}

/// Reports whether every link of an item's clip chain is a rectangle without radii.
fn hard_chain(scene: &Scene, chunk: &RecordedChunk, clip: ClipRef) -> bool {
    let node_chain = |slot: Option<u32>| {
        let mut current = slot;
        let mut steps = 0;
        while let Some(clip) = current {
            let Some(record) = scene.clips.at(clip) else {
                return false;
            };
            if !square(&record.node.shape) {
                return false;
            }
            current = record.node.parent.map(|parent| parent.slot());
            steps += 1;
            if steps > scene.clips.span() {
                return false;
            }
        }
        true
    };

    if clip == ClipRef::NONE {
        return true;
    }
    if clip == ClipRef::INHERIT {
        return node_chain(chunk.props.clip.map(|clip| clip.slot()));
    }
    if clip.local_index().is_some() {
        // Local chains are rare under a backing colour, so they are refused.
        return false;
    }
    node_chain(Some(clip.0))
}
