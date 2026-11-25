//! Clip chains: resolving them from items and effects, and their device bounding boxes.

use iceblink::{ClipId, ClipRef, ClipShape, LocalParent, LocalRect, MaskMode, Matrix4};

use crate::naive::frame::slots::LinkId;
use crate::naive::geometry::{Bounds, plane_affine};
use crate::naive::scene::{Chunk, Client};
use crate::naive::space::Worlds;

/// One link of a clip chain, with its shape measured in the link's transform node.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Link {
    pub(crate) id: LinkId,
    /// The slot of the transform node the shape is measured in.
    pub(crate) space: u32,
    pub(crate) shape: LinkShape,
}

/// The shape of a link.
///
/// The rectangle of a chunk-local clip already includes the chunk offset.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LinkShape {
    Rounded {
        rect: LocalRect,
        radii: [f32; 8],
        shapes: [f32; 4],
    },
    Mask {
        rect: LocalRect,
        /// The image's slot in the client.
        image: u32,
        source: [f32; 4],
        mode: MaskMode,
    },
}

/// Appends the chain of the clip node `id`, outermost link first.
pub(crate) fn node_chain(client: &Client, id: ClipId, chain: &mut Vec<Link>) {
    slot_chain(client, id.slot(), chain);
}

/// Appends the chain of the clip node at `slot`, outermost link first.
fn slot_chain(client: &Client, slot: u32, chain: &mut Vec<Link>) {
    let start = chain.len();
    let mut at = Some(slot);
    while let Some(slot) = at {
        let Some(node) = client.clips.at(slot) else {
            break;
        };
        chain.push(Link {
            id: LinkId::Node(client.ids.client(), slot),
            space: node.space.slot(),
            shape: shape(&node.shape, [0.0; 2]),
        });
        at = node.parent.map(|parent| parent.slot());
    }

    chain[start..].reverse();
}

/// Appends the chain of an item of `chunk` with the clip reference `clip`, outermost link first.
pub(crate) fn item_chain(client: &Client, chunk: &Chunk, clip: ClipRef, chain: &mut Vec<Link>) {
    if clip == ClipRef::NONE {
        return;
    }

    if clip == ClipRef::INHERIT {
        if let Some(id) = chunk.props.clip {
            node_chain(client, id, chain);
        }
        return;
    }

    if let Some(index) = clip.local_index() {
        local_chain(client, chunk, index, chain);
        return;
    }

    slot_chain(client, clip.0, chain);
}

/// Appends the chain of the chunk-local clip at `index`, outermost link first.
fn local_chain(client: &Client, chunk: &Chunk, index: u32, chain: &mut Vec<Link>) {
    let clips = &chunk.recorded.local_clips;
    let mut links = Vec::new();
    let mut at = Some(index);
    let mut parent = LocalParent::None;
    while let Some(index) = at {
        let Some(local) = clips.get(index as usize) else {
            break;
        };
        links.push(Link {
            id: LinkId::Local(client.ids.client(), chunk.recorded.key, index),
            space: chunk.props.space.slot(),
            shape: shape(&local.shape, chunk.offset),
        });
        at = match local.parent {
            LocalParent::Local(earlier) => Some(earlier),
            other => {
                parent = other;
                None
            }
        };
    }

    match parent {
        LocalParent::Chunk => {
            if let Some(id) = chunk.props.clip {
                node_chain(client, id, chain);
            }
        }
        LocalParent::Node(id) => node_chain(client, id, chain),
        LocalParent::None | LocalParent::Local(_) => {}
    }

    links.reverse();
    chain.extend(links);
}

fn shape(shape: &ClipShape, offset: [f32; 2]) -> LinkShape {
    let moved = |rect: &LocalRect| [rect[0] + offset[0], rect[1] + offset[1], rect[2], rect[3]];
    match shape {
        ClipShape::RoundedRect {
            rect,
            radii,
            shapes,
        } => LinkShape::Rounded {
            rect: moved(rect),
            radii: *radii,
            shapes: *shapes,
        },
        ClipShape::Mask {
            image,
            rect,
            source,
            mode,
        } => LinkShape::Mask {
            rect: moved(rect),
            image: image.slot(),
            source: *source,
            mode: *mode,
        },
    }
}

/// Returns the device bounding box of a chain: the intersection of its links' boxes.
///
/// A link whose rectangle cannot be mapped to the device bounds nothing.
pub(crate) fn chain_box(chain: &[Link], worlds: &mut Worlds<'_>) -> Bounds {
    let mut bounds = Bounds {
        x0: f32::NEG_INFINITY,
        y0: f32::NEG_INFINITY,
        x1: f32::INFINITY,
        y1: f32::INFINITY,
    };

    for link in chain {
        if let Some(link_box) = link_box(link, worlds) {
            bounds = bounds.intersect(link_box);
        }
    }

    bounds
}

/// Returns the device box of one link, grown so that it holds every pixel the link covers.
fn link_box(link: &Link, worlds: &mut Worlds<'_>) -> Option<Bounds> {
    let world = worlds.of(link.space)?;
    match &link.shape {
        LinkShape::Rounded { rect, radii, .. } => {
            let rect = Bounds::of(*rect);
            if is_hard_edge(&world, radii) {
                return rect.mapped(&world);
            }

            // A soft link covers up to half a local unit outside its rectangle.
            rect.outset(0.5).mapped(&world)
        }
        LinkShape::Mask { rect, .. } => Bounds::of(*rect).mapped(&world),
    }
}

/// Reports whether a rounded link is a hard edge: no radii, and a world matrix that maps its
/// rectangle to an axis-aligned device rectangle.
fn is_hard_edge(world: &Matrix4, radii: &[f32; 8]) -> bool {
    if radii.iter().any(|radius| *radius != 0.0) {
        return false;
    }

    let Some(affine) = plane_affine(world) else {
        return false;
    };

    let [a, b, c, d, _, _] = affine.as_coeffs();
    (b == 0.0 && c == 0.0) || (a == 0.0 && d == 0.0)
}
