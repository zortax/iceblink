//! Clip chains as the frame clip table carries them.

use iceblink::{ClipEntry, ClipLinkEntry, ClipShape, FrameClipShape, LocalParent, RecordedChunk};
use smallvec::SmallVec;

use crate::scene::Scene;
use crate::scene::record::NO_FRAME_SLOT;
use crate::shared_content::{Content, image_key};

/// Returns one link of a clip of `scene`, measured in frame space slot `space`, moved by
/// `offset`.
fn link(
    scene: &Scene,
    content: &mut Content,
    space: u32,
    shape: &ClipShape,
    offset: [f32; 2],
) -> ClipLinkEntry {
    let moved = |rect: [f32; 4]| [rect[0] + offset[0], rect[1] + offset[1], rect[2], rect[3]];

    let shape = match shape {
        ClipShape::RoundedRect {
            rect,
            radii,
            shapes,
        } => FrameClipShape::RoundedRect {
            rect: moved(*rect),
            radii: *radii,
            shapes: *shapes,
        },
        ClipShape::Mask {
            image,
            rect,
            source,
            mode,
        } => FrameClipShape::Mask {
            rect: moved(*rect),
            tile: content.mask_tile(image_key(scene.client, image.slot()), *source),
            mode: *mode,
        },
    };
    ClipLinkEntry { space, shape }
}

/// Appends the links of the chain of clip node `slot`, outermost first.
fn push_chain(
    scene: &Scene,
    content: &mut Content,
    slot: u32,
    links: &mut SmallVec<[ClipLinkEntry; 2]>,
) {
    let mut chain: SmallVec<[u32; 4]> = SmallVec::new();
    let mut current = Some(slot);
    while let Some(clip) = current {
        let Some(record) = scene.clips.at(clip) else {
            break;
        };
        // A chain cannot be longer than the slab without a cycle, which commits reject.
        if chain.len() > scene.clips.span() {
            break;
        }
        chain.push(clip);
        current = record.node.parent.map(|parent| parent.slot());
    }

    for &clip in chain.iter().rev() {
        let Some(record) = scene.clips.at(clip) else {
            continue;
        };
        let space = scene
            .transforms
            .at(record.node.space.slot())
            .map_or(NO_FRAME_SLOT, |space| space.links.frame);
        links.push(link(scene, content, space, &record.node.shape, [0.0; 2]));
    }
}

/// Returns the frame entry of the chain of clip node `slot`.
pub(crate) fn node_chain(scene: &Scene, content: &mut Content, slot: u32) -> ClipEntry {
    let mut links = SmallVec::new();
    push_chain(scene, content, slot, &mut links);

    ClipEntry { links }
}

/// Returns the frame entry of chunk-local clip `index` of `chunk`, at `offset`.
pub(crate) fn local_chain(
    scene: &Scene,
    content: &mut Content,
    chunk: &RecordedChunk,
    index: usize,
    offset: [f32; 2],
) -> ClipEntry {
    let mut own: SmallVec<[usize; 4]> = SmallVec::new();
    let mut outer: Option<u32> = None;
    let mut current = Some(index);
    while let Some(local) = current {
        let Some(clip) = chunk.local_clips.get(local) else {
            break;
        };
        own.push(local);
        current = match clip.parent {
            LocalParent::Local(parent) if (parent as usize) < local => Some(parent as usize),
            LocalParent::Local(_) | LocalParent::None => None,
            LocalParent::Chunk => {
                outer = chunk.props.clip.map(|clip| clip.slot());
                None
            }
            LocalParent::Node(id) => {
                outer = Some(id.slot());
                None
            }
        };
    }

    let mut links = SmallVec::new();
    if let Some(outer) = outer {
        push_chain(scene, content, outer, &mut links);
    }

    let space = scene
        .transforms
        .at(chunk.props.space.slot())
        .map_or(NO_FRAME_SLOT, |space| space.links.frame);
    for &local in own.iter().rev() {
        links.push(link(
            scene,
            content,
            space,
            &chunk.local_clips[local].shape,
            offset,
        ));
    }
    ClipEntry { links }
}

/// Returns the chain `inner` with the links of `outer` added outside it, or `outer` alone without
/// `inner`.
pub(crate) fn combined(outer: &ClipEntry, inner: Option<&ClipEntry>) -> ClipEntry {
    let mut links = outer.links.clone();
    if let Some(inner) = inner {
        links.extend(inner.links.iter().cloned());
    }
    ClipEntry { links }
}
