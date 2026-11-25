//! The device bounds of clip chains.

use iceblink::{ClipRef, ClipShape, LocalParent, LocalRect, Matrix4, RecordedChunk};

use crate::geometry::bounds::{DeviceBox, axis_scale, keeps_axes, map};
use crate::scene::Scene;

/// How far a soft clip edge ramps on either side, in the link's own units.
pub(crate) const RAMP: f32 = 0.5;

/// Returns the rectangle a clip shape admits, in its own space.
pub(crate) fn shape_rect(shape: &ClipShape) -> LocalRect {
    match shape {
        ClipShape::RoundedRect { rect, .. } | ClipShape::Mask { rect, .. } => *rect,
    }
}

/// Returns the device box of a clip link of `shape` over `rect` in a space with world matrix
/// `world`.
///
/// Rounded outwards to whole pixels, the box holds every pixel whose centre the link covers. When
/// the space enlarges the link, the ramp of a soft edge counts as covered.
pub(crate) fn link_box(world: &Matrix4, shape: &ClipShape, rect: LocalRect) -> DeviceBox {
    let plain = map(world, rect);
    let ClipShape::RoundedRect { .. } = shape else {
        return plain;
    };
    if hard_edge(world, shape) {
        return plain;
    }
    if let Some([sx, sy, ..]) = axis_scale(world)
        && sx.abs() <= 1.0
        && sy.abs() <= 1.0
    {
        return plain;
    }

    let [x, y, width, height] = rect;
    let grown = [x - RAMP, y - RAMP, width + 2.0 * RAMP, height + 2.0 * RAMP];
    // Pixel centres lie half a pixel inside their pixels.
    plain.union(map(world, grown).outset([-0.5; 4]))
}

/// Reports whether a clip link of `shape` in a space with world matrix `world` has a hard edge: a
/// rectangle without radii that the space maps to an axis-aligned device rectangle.
pub(crate) fn hard_edge(world: &Matrix4, shape: &ClipShape) -> bool {
    match shape {
        ClipShape::RoundedRect { radii, .. } => {
            radii.iter().all(|&radius| radius == 0.0) && keeps_axes(world)
        }
        ClipShape::Mask { .. } => false,
    }
}

/// Returns the device bounds of the chain of clip node `slot`, from its link and its parent's
/// stored bounds.
pub(crate) fn node_bounds(scene: &Scene, slot: u32) -> DeviceBox {
    let Some(record) = scene.clips.at(slot) else {
        return DeviceBox::EVERYWHERE;
    };
    let parent = record
        .node
        .parent
        .and_then(|parent| scene.clips.at(parent.slot()))
        .map_or(DeviceBox::EVERYWHERE, |parent| parent.bounds);
    let world = scene
        .transforms
        .at(record.node.space.slot())
        .map(|space| space.world);

    match world {
        Some(world) => {
            let shape = &record.node.shape;
            link_box(&world, shape, shape_rect(shape)).intersect(parent)
        }
        None => parent,
    }
}

/// Fills `out` with the device bounds of every chunk-local clip of `chunk` at `offset`, in the
/// order of the chunk's local clips.
pub(crate) fn local_bounds(
    scene: &Scene,
    chunk: &RecordedChunk,
    offset: [f32; 2],
    out: &mut Vec<DeviceBox>,
) {
    out.clear();
    let world = scene
        .transforms
        .at(chunk.props.space.slot())
        .map(|space| space.world);
    let chunk_clip = chunk
        .props
        .clip
        .and_then(|clip| scene.clips.at(clip.slot()))
        .map_or(DeviceBox::EVERYWHERE, |clip| clip.bounds);

    for clip in &chunk.local_clips {
        let parent = match clip.parent {
            LocalParent::Chunk => chunk_clip,
            LocalParent::Local(index) => out
                .get(index as usize)
                .copied()
                .unwrap_or(DeviceBox::EVERYWHERE),
            LocalParent::Node(id) => scene
                .clips
                .at(id.slot())
                .map_or(DeviceBox::EVERYWHERE, |clip| clip.bounds),
            LocalParent::None => DeviceBox::EVERYWHERE,
        };

        let mut rect = shape_rect(&clip.shape);
        rect[0] += offset[0];
        rect[1] += offset[1];
        let own = world.map_or(DeviceBox::EVERYWHERE, |world| {
            link_box(&world, &clip.shape, rect)
        });
        out.push(own.intersect(parent));
    }
}

/// Returns the device bounds an item's clip reference admits.
pub(crate) fn item_bounds(
    scene: &Scene,
    chunk: &RecordedChunk,
    locals: &[DeviceBox],
    clip: ClipRef,
) -> DeviceBox {
    if clip == ClipRef::NONE {
        return DeviceBox::EVERYWHERE;
    }
    if clip == ClipRef::INHERIT {
        return chunk
            .props
            .clip
            .and_then(|clip| scene.clips.at(clip.slot()))
            .map_or(DeviceBox::EVERYWHERE, |clip| clip.bounds);
    }
    if let Some(index) = clip.local_index() {
        return locals
            .get(index as usize)
            .copied()
            .unwrap_or(DeviceBox::EVERYWHERE);
    }
    scene
        .clips
        .at(clip.0)
        .map_or(DeviceBox::EVERYWHERE, |clip| clip.bounds)
}
