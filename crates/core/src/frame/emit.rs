//! Emitting the items a frame draws one at a time: paths, external textures and backdrops.

use iceblink::{
    ChunkKey, DeviceRect, FrameBackdrop, FrameExternal, FramePath, ItemKind, kurbo::Affine,
    read_extent,
};

use crate::frame::filters::frame_filters;
use crate::frame::lower::{Slots, View};
use crate::frame::walk::Walk;
use crate::geometry::bounds::{DeviceBox, INK_LIMIT, map, of_pixels};
use crate::scene::chunk::LoweredPath;

/// Emits the external texture or backdrop at `position` in the ops of chunk `key`, drawn on its
/// own through the clip chain `context`.
///
/// Items of other kinds emit nothing.
pub(crate) fn single(
    walk: &mut Walk<'_>,
    key: ChunkKey,
    position: usize,
    context: u32,
    ink: DeviceRect,
) {
    let scene = &*walk.scene;
    let Some(record) = scene.chunks.get(&key) else {
        return;
    };
    let chunk = &record.recorded;
    let Some(op) = chunk.ops.get(position) else {
        return;
    };
    let slots = Slots {
        scene: View::of(scene),
        chunk,
        locals: &record.derived.local_frames,
        context,
    };
    let offset = record.offset;

    let index = op.index as usize;
    match op.kind {
        ItemKind::EXTERNAL => {
            let Some(item) = chunk.externals.get(index) else {
                return;
            };
            let Some(texture) = scene.textures.at(item.texture) else {
                return;
            };
            let mut quad = *item;
            quad.bounds[0] += offset[0];
            quad.bounds[1] += offset[1];
            quad.clip = slots.clip(walk.tables, quad.clip);
            quad.space = slots.space(quad.space);
            walk.frame.externals.push(FrameExternal {
                quad,
                client: scene.client,
                texture: texture.id,
            });
            walk.scratch.emission.push_external(ink);
        }
        ItemKind::BACKDROP => {
            let Some(item) = chunk.backdrops.get(index) else {
                return;
            };
            let Some(chain) = chunk.filters.get(item.filters as usize) else {
                return;
            };
            let (_, world) = slots.world(item.space);
            let mut backdrop = *item;
            backdrop.bounds[0] += offset[0];
            backdrop.bounds[1] += offset[1];
            let bounds = map(&world, backdrop.bounds).rect();
            backdrop.clip = slots.clip(walk.tables, backdrop.clip);
            backdrop.space = slots.space(backdrop.space);
            let filters = frame_filters(scene, chain);
            let source = read_extent(bounds, chain);
            let reads_outside = DeviceBox::of(source)
                .pixels(walk.frame_bounds())
                .is_some_and(|read| read.intersection(ink) != Some(read));
            walk.frame.backdrops.push(FrameBackdrop {
                backdrop,
                bounds,
                source,
                filters: filters.clone(),
            });
            walk.note_filter_shaders(&filters);
            walk.scratch.emission.push_backdrop(ink, reads_outside);
        }
        _ => {}
    }
}

/// Emits a frame path a lowering holds, moved by `by` device pixels and cut to `ink`.
pub(crate) fn lowered_path(
    walk: &mut Walk<'_>,
    lowered: &LoweredPath,
    by: [f64; 2],
    ink: DeviceRect,
) {
    let mut path = lowered.path.clone();
    if by != [0.0; 2] {
        path.transform = Affine::translate((by[0], by[1])) * path.transform;
        let [x, y] = by.map(|by| by as f32);
        let Some(moved) = of_pixels(path.ink).outset([-x, -y, x, y]).pixels(INK_LIMIT) else {
            return;
        };
        path.ink = moved;
    }
    push_path(walk, path, lowered.clip, ink);
}

/// Adds a frame path drawn through the frame clip slot `clip`, with its ink cut to `ink`.
fn push_path(walk: &mut Walk<'_>, mut path: FramePath, clip: u32, ink: DeviceRect) {
    let reach = path.ink;
    let Some(ink) = reach.intersection(ink) else {
        return;
    };
    path.ink = ink;
    walk.frame.paths.push(path);
    walk.scratch.path_clips.push(clip);
    walk.scratch.emission.push_path(ink, reach, clip);
}
