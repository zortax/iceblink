//! A random scene and random edits of it: quads, glyph runs and paths in chunks placed among
//! groups, a scroll port with its clip, a moving space, paints and chunk offsets.

mod build;
mod edit;
mod items;

use iceblink::{
    Anchor, ChunkFlags, ChunkKey, ChunkProps, ClipId, EffectId, Matrix4, OrderId, PaintId,
    ScrollId, TransformId, TransformNode,
};

use crate::conformance::end_to_end::numbers::Numbers;

pub(crate) use self::build::scene;
pub(crate) use self::edit::edit;

/// How many chunks a scene records.
const CHUNKS: u32 = 12;

/// The order nodes chunks are placed in: a plain list, a filtered group, an opacity group, a
/// blended group, and a blurred group inside the opacity group, after its chunks.
const NODES: usize = 5;

/// The node the nested group sits in.
const OUTER: usize = 2;

/// The scroll port, over the left half of the output.
const PORT: [f32; 4] = [4.0, 4.0, 64.0, 120.0];

/// The ids of a random scene in one compositor.
#[derive(Clone, Debug)]
pub(crate) struct Ids {
    space: TransformId,
    scroll: ScrollId,
    /// The transform node that edits move.
    free: TransformId,
    /// The scroll port's clip.
    port: ClipId,
    paints: [PaintId; 3],
    nodes: [OrderId; NODES],
    /// The effect of every node but the list.
    effects: [EffectId; NODES - 1],
    /// The space of each chunk, by its part: the scrolled content, the free space, the root.
    spaces: [TransformId; 3],
}

/// The state of a scene the random edits know.
#[derive(Clone, Debug)]
pub(crate) struct Model {
    numbers: Numbers,
    /// The parts of the chunks each order node holds, in order.
    held: [Vec<u32>; NODES],
    offset: f32,
    /// The blend modes effects may use, as a bit per `BlendMode` value.
    blend_modes: u32,
}

impl Model {
    /// Returns the model of a scene not built yet, from `seed`, whose effects use only the blend
    /// modes of `blend_modes` and `NORMAL` in place of any other.
    pub(crate) fn new(seed: u64, blend_modes: u32) -> Self {
        Self {
            numbers: Numbers(seed.wrapping_mul(104_729).wrapping_add(7)),
            held: Default::default(),
            offset: 0.0,
            blend_modes,
        }
    }

    /// Returns how many edits the next step makes.
    pub(crate) fn edits(&mut self) -> u64 {
        1 + self.numbers.below(3)
    }

    /// Returns the node that holds chunk `part` and its position there, if a node does.
    fn place(&self, part: u32) -> Option<(usize, usize)> {
        self.held.iter().enumerate().find_map(|(node, held)| {
            held.iter()
                .position(|&other| other == part)
                .map(|at| (node, at))
        })
    }
}

fn key(part: u32) -> ChunkKey {
    ChunkKey::new(10, part)
}

fn identity(parent: Option<TransformId>) -> TransformNode {
    TransformNode {
        parent,
        local: Matrix4::IDENTITY,
        scroll: None,
        anchor: Anchor::Scrolls,
    }
}

fn props(space: TransformId, clip: Option<ClipId>) -> ChunkProps {
    ChunkProps {
        space,
        clip,
        flags: ChunkFlags::default(),
    }
}

/// Returns a random node for chunk `part`.
///
/// Scrolled content goes to the list three times in four, where a scroll can shift it.
fn node_for(numbers: &mut Numbers, part: u32) -> usize {
    match numbers.below(4 * NODES as u64) as usize {
        node if node < NODES => node,
        _ if part.is_multiple_of(3) => 0,
        node => node % NODES,
    }
}
