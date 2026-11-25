//! The stored form of every node, slot and resource.

use std::sync::Arc;

use iceblink::{
    ChunkKey, ClipNode, DeviceRect, EffectNode, ImageDesc, Matrix4, OrderEntry, OrderId, OrderNode,
    Paint, ParamsBlock, ScrollNode, ShaderDesc, TextureId, TransformNode,
};
use rustc_hash::FxHashSet;
use smallvec::SmallVec;

use crate::derive::carry::whole_pixels;
use crate::geometry::bounds::{DeviceBox, INK_LIMIT};
use crate::scene::follows::{ClipSplit, Follows};
use crate::scene::inks::{NO_PIXELS, NodeInk};
use crate::scene::reaches::Reaches;
use crate::scene::rows::Rows;

/// The frame slot of an object that has none yet.
pub(crate) const NO_FRAME_SLOT: u32 = u32::MAX;

/// The bookkeeping every stored object shares.
#[derive(Debug)]
pub(crate) struct Links {
    /// How many live nodes, order entries and output roots refer to the object.
    pub(crate) refs: u32,
    /// The chunks whose items, props or local clips refer to the object.
    pub(crate) users: FxHashSet<ChunkKey>,
    /// The object's slot in its frame table, or `NO_FRAME_SLOT`.
    pub(crate) frame: u32,
}

impl Default for Links {
    fn default() -> Self {
        Self {
            refs: 0,
            users: FxHashSet::default(),
            frame: NO_FRAME_SLOT,
        }
    }
}

/// A transform node and its resolved matrices.
#[derive(Debug)]
pub(crate) struct TransformRecord {
    pub(crate) node: TransformNode,
    pub(crate) links: Links,
    /// The world matrix, from local space to device space.
    pub(crate) world: Matrix4,
    /// The world matrix with every scroll offset taken as zero, which fixed descendants build on.
    pub(crate) still: Matrix4,
    /// The transform nodes whose parent this node is.
    pub(crate) children: Vec<u32>,
    /// The clip nodes measured in this node's space.
    pub(crate) clips: Vec<u32>,
    /// The scroll offsets the world matrix follows.
    pub(crate) follows: Follows,
    /// The scroll offsets the still matrix follows.
    pub(crate) still_follows: Follows,
    /// The users no scroll node carries, kept while the node follows a scroll offset.
    pub(crate) loose: FxHashSet<ChunkKey>,
    /// Whether the node is moving, as [`Motion`](crate::scene::motion::Motion) decides.
    pub(crate) moving: bool,
}

impl TransformRecord {
    pub(crate) fn new(node: TransformNode) -> Self {
        Self {
            node,
            links: Links::default(),
            world: Matrix4::IDENTITY,
            still: Matrix4::IDENTITY,
            children: Vec::new(),
            clips: Vec::new(),
            follows: Follows::Still,
            still_follows: Follows::Still,
            loose: FxHashSet::default(),
            moving: false,
        }
    }
}

/// A scroll node.
#[derive(Debug)]
pub(crate) struct ScrollRecord {
    pub(crate) node: ScrollNode,
    pub(crate) links: Links,
    /// The transform nodes that apply this node's offset or stick inside its port.
    pub(crate) dependents: Vec<u32>,
    /// How far the content the node carries has moved on the device, in whole pixels.
    pub(crate) travel: [i32; 2],
    /// The order nodes the node does not carry that hold an entry it carries.
    pub(crate) watchers: FxHashSet<u32>,
    /// The overscroll displacement, in the node's units, which dependents apply with the offset.
    pub(crate) overscroll: [f32; 2],
}

impl ScrollRecord {
    pub(crate) fn new(node: ScrollNode) -> Self {
        Self {
            node,
            links: Links::default(),
            dependents: Vec::new(),
            travel: [0; 2],
            watchers: FxHashSet::default(),
            overscroll: [0.0; 2],
        }
    }
}

/// A clip node.
#[derive(Debug)]
pub(crate) struct ClipRecord {
    pub(crate) node: ClipNode,
    pub(crate) links: Links,
    /// The clip nodes whose parent this node is.
    pub(crate) children: Vec<u32>,
    /// The device box of the whole chain.
    pub(crate) bounds: DeviceBox,
    /// The effect nodes whose clip this node is.
    pub(crate) effects: Vec<u32>,
    /// The device bounds of the whole chain, split by whether each link follows a scroll offset.
    pub(crate) split: ClipSplit,
    /// The whole pixels of the split's still bounds, or `None` when they are empty or an edge
    /// is not whole.
    pub(crate) still: Option<DeviceRect>,
    /// The users no scroll node carries, kept while a link of the chain follows a scroll offset.
    pub(crate) loose: FxHashSet<ChunkKey>,
    /// The users with the reach of each, kept once a write of the clip left many users alone.
    pub(crate) reaches: Option<Reaches>,
}

impl ClipRecord {
    pub(crate) fn new(node: ClipNode) -> Self {
        Self {
            node,
            links: Links::default(),
            children: Vec::new(),
            bounds: DeviceBox::EVERYWHERE,
            effects: Vec::new(),
            split: ClipSplit::OPEN,
            still: whole_pixels(ClipSplit::OPEN.still),
            loose: FxHashSet::default(),
            reaches: None,
        }
    }

    /// Returns the whole pixels of the chain's still bounds, rounded out when an edge is not
    /// whole.
    #[inline]
    pub(crate) fn still_pixels(&self) -> DeviceRect {
        self.still
            .or_else(|| self.split.still.pixels(INK_LIMIT))
            .unwrap_or(NO_PIXELS)
    }
}

/// An effect node.
#[derive(Debug)]
pub(crate) struct EffectRecord {
    pub(crate) node: EffectNode,
    pub(crate) links: Links,
    /// The order nodes that apply this effect.
    pub(crate) orders: Vec<u32>,
}

/// An order node, its sequence and where it is placed.
#[derive(Debug)]
pub(crate) struct OrderRecord {
    pub(crate) id: OrderId,
    pub(crate) node: OrderNode,
    pub(crate) entries: Vec<OrderEntry>,
    pub(crate) links: Links,
    /// The ink of the subtree: the union of its item inks, or the written region when the
    /// node's effect isolates.
    pub(crate) ink: NodeInk,
    /// The scroll nodes that carry one of the node's entries without carrying the node.
    pub(crate) watching: SmallVec<[u32; 1]>,
    /// The vertical extents of the entries with an ink, in content space for a carried node and on
    /// the device for another, when they are ordered.
    pub(crate) rows: Option<Rows>,
    /// Whether the cached inks are out of date.
    pub(crate) dirty: bool,
    /// Whether a node in the subtree is out of date. The cached inks stay valid unless the ink of
    /// a node entry changes.
    pub(crate) above: bool,
    /// Whether chunk entries changed only the still pixels they are cut to. The cached inks stay
    /// valid except for their carry.
    pub(crate) recarry: bool,
    /// The refresh of order node inks that last changed the cached ink.
    pub(crate) changed_in: u32,
    /// The refresh of order node inks that last changed only the carry of the cached ink.
    pub(crate) recarried_in: u32,
    /// The refresh of order node inks that last marked the node stale or above a stale node.
    pub(crate) visited_in: u32,
}

impl OrderRecord {
    pub(crate) fn new(id: OrderId, node: OrderNode) -> Self {
        Self {
            id,
            node,
            entries: Vec::new(),
            links: Links::default(),
            ink: NodeInk::default(),
            watching: SmallVec::new(),
            rows: None,
            dirty: true,
            above: false,
            recarry: false,
            changed_in: 0,
            recarried_in: 0,
            visited_in: 0,
        }
    }
}

/// A paint slot.
#[derive(Debug)]
pub(crate) struct PaintRecord {
    pub(crate) paint: Paint,
    pub(crate) links: Links,
}

/// A shader parameter slot.
#[derive(Debug)]
pub(crate) struct ParamsRecord {
    pub(crate) block: ParamsBlock,
    pub(crate) links: Links,
}

/// An image, whose pixels the content store keeps.
#[derive(Debug)]
pub(crate) struct ImageRecord {
    pub(crate) desc: ImageDesc,
    pub(crate) links: Links,
    /// The clip nodes that mask with this image.
    pub(crate) masks: Vec<u32>,
}

/// An external texture slot.
#[derive(Debug)]
pub(crate) struct TextureRecord {
    pub(crate) id: TextureId,
    pub(crate) links: Links,
}

/// An application shader.
#[derive(Debug)]
pub(crate) struct ShaderRecord {
    pub(crate) desc: Arc<ShaderDesc>,
    pub(crate) links: Links,
}
