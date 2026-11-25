//! The changes a commit made to primary state, with the old values needed to undo them.

mod undo;

use iceblink::damage::DamageSet;
use iceblink::{
    AnyId, ArenaRange, ChunkKey, ChunkProps, ClientId, ImageId, ImageWrite, OrderEntry, OrderId,
    OrderNode, OutputId, TextureId,
};
use smallvec::SmallVec;

use crate::scene::Scene;
use crate::scene::chunk::ChunkRecord;
use crate::scene::record::{
    ClipRecord, EffectRecord, ImageRecord, OrderRecord, PaintRecord, ParamsRecord, ScrollRecord,
    ShaderRecord, TextureRecord, TransformRecord,
};
use crate::scene::slab::Vacancy;
use crate::scene::stored::Stored;
use crate::tier::stage::TierStage;

/// One change to one stored object, which keeps a record it removed as `T`.
#[derive(Debug)]
pub(crate) struct Change<R: Stored, T = R> {
    pub(crate) slot: u32,
    /// The index of the command that made the change.
    pub(crate) command: u32,
    pub(crate) op: Op<R, T>,
}

/// What a change did, holding what is needed to undo it.
#[derive(Debug)]
pub(crate) enum Op<R: Stored, T = R> {
    /// The object was created, and the slot held `Vacancy` before.
    Created(Vacancy),
    /// The object was removed, and this keeps the record taken out.
    Removed(T),
    /// The object's value was written, and this is the value before.
    Wrote(R::Value),
}

/// One change to an order node, which keeps a removed record as its index in
/// `Journal::removed_orders`.
pub(crate) type OrderNodeChange = Change<OrderRecord, u32>;

/// One change to the chunk store.
#[derive(Debug)]
pub(crate) enum ChunkChange {
    /// A chunk was recorded or removed, and this is what the key held before.
    Replaced {
        key: ChunkKey,
        command: u32,
        /// The index in `Journal::replaced` of the record the key held, if it held one.
        before: Option<u32>,
    },
    /// A chunk's offset was written, and this is the offset before.
    Offset { key: ChunkKey, before: [f32; 2] },
    /// A chunk's props were written, and these are the props before.
    Props { key: ChunkKey, before: ChunkProps },
}

/// One change to the order tree: to an order node, or to a sequence.
#[derive(Debug)]
pub(crate) enum OrderChange {
    /// A change to an order node, kept in `Journal::order_nodes` in the same order.
    Node,
    Splice(Splice),
}

/// One splice, with the entries it removed.
#[derive(Debug)]
pub(crate) struct Splice {
    pub(crate) node: u32,
    pub(crate) command: u32,
    pub(crate) at: u32,
    /// The removed entries, as a range of `Journal::removed_entries`.
    pub(crate) removed: ArenaRange,
    /// Whether the node had ink before the commit. Its removed entries had ink only if it did.
    pub(crate) inked: bool,
    pub(crate) inserted: u32,
    /// The inserted entries, as a range of the transaction's entry list.
    pub(crate) entries: ArenaRange,
}

/// One write of an output's root, with the root before.
#[derive(Debug)]
pub(crate) struct RootWrite {
    pub(crate) output: OutputId,
    pub(crate) command: u32,
    pub(crate) before: Option<(ClientId, OrderId)>,
}

/// Work that only happens once the commit succeeds.
#[derive(Debug, Default)]
pub(crate) struct Deferred {
    pub(crate) image_writes: Vec<(ImageId, ImageWrite)>,
    pub(crate) texture_frames: Vec<TextureId>,
}

impl Deferred {
    pub(crate) fn is_empty(&self) -> bool {
        self.image_writes.is_empty() && self.texture_frames.is_empty()
    }
}

/// Every change a commit made, in the order it made them, per kind.
#[derive(Debug, Default)]
pub(crate) struct Journal {
    pub(crate) transforms: Vec<Change<TransformRecord>>,
    pub(crate) scrolls: Vec<Change<ScrollRecord>>,
    pub(crate) clips: Vec<Change<ClipRecord>>,
    pub(crate) effects: Vec<Change<EffectRecord>>,
    pub(crate) orders: Vec<OrderChange>,
    /// The changes to order nodes, in the order `OrderChange::Node` marks them in `orders`.
    pub(crate) order_nodes: Vec<OrderNodeChange>,
    /// The order node records the commit removed, in order.
    pub(crate) removed_orders: Vec<OrderRecord>,
    /// The order nodes with an effect before or after one of their changes in `order_nodes`.
    effect_orders: Vec<u32>,
    /// The entries splices removed, which `Splice::removed` refers to by range.
    pub(crate) removed_entries: Vec<OrderEntry>,
    /// The entries a splice placed while sequences held them already. A chunk key whose removal
    /// waits in `unplaced` counts as held.
    pub(crate) doubled: Vec<OrderEntry>,
    /// The chunk keys splices removed and the nodes that held them, noted in the scene's parents
    /// once every command is applied.
    pub(crate) unplaced: Vec<(ChunkKey, u32)>,
    /// The ids the commit removed, to free once it succeeds.
    pub(crate) freed: Vec<AnyId>,
    pub(crate) paints: Vec<Change<PaintRecord>>,
    pub(crate) params: Vec<Change<ParamsRecord>>,
    pub(crate) images: Vec<Change<ImageRecord>>,
    pub(crate) textures: Vec<Change<TextureRecord>>,
    pub(crate) shaders: Vec<Change<ShaderRecord>>,
    pub(crate) chunks: Vec<ChunkChange>,
    /// The chunk records replacements took out, which `ChunkChange::Replaced` refers to by index.
    pub(crate) replaced: Vec<Option<ChunkRecord>>,
    pub(crate) roots: Vec<RootWrite>,
    pub(crate) deferred: Deferred,
    /// The damage the transaction declared, by output.
    pub(crate) declared: Vec<(OutputId, DamageSet)>,
    /// The commit's changes to the compositor-driven tier.
    pub(crate) tier: TierStage,
    /// The slots of the transform nodes that came to rest, whose users are drawn again.
    pub(crate) rested: Vec<u32>,
}

impl Journal {
    /// Keeps a record a replacement took out, and returns the index it is kept at.
    pub(crate) fn keep(&mut self, record: Option<ChunkRecord>) -> Option<u32> {
        let record = record?;
        self.replaced.push(Some(record));
        Some(self.replaced.len() as u32 - 1)
    }

    /// Returns a record a replacement took out, by the index `keep` returned.
    pub(crate) fn before(&self, index: Option<u32>) -> Option<&ChunkRecord> {
        self.replaced.get(index? as usize)?.as_ref()
    }

    /// Notes a change to an order node, which `effect` reports to have an effect before or after
    /// it.
    pub(crate) fn order_node(&mut self, change: OrderNodeChange, effect: bool) {
        let slot = change.slot;

        self.orders.push(OrderChange::Node);
        self.order_nodes.push(change);
        if effect {
            self.effect_orders.push(slot);
        }
    }

    /// Keeps an order node record the commit removed, and returns the index it is kept at.
    pub(crate) fn keep_order(&mut self, record: OrderRecord) -> u32 {
        self.removed_orders.push(record);
        self.removed_orders.len() as u32 - 1
    }

    /// Returns the record a removal of an order node took out, by the index `keep_order`
    /// returned.
    pub(crate) fn removed_order(&self, index: u32) -> Option<&OrderRecord> {
        self.removed_orders.get(index as usize)
    }

    /// Returns each order node with an effect before or after one of its changes, with its node
    /// before the commit, or `None` when the commit created it.
    pub(crate) fn effect_order_nets(&self) -> SmallVec<[(u32, Option<OrderNode>); 4]> {
        let mut nets: SmallVec<[(u32, Option<OrderNode>); 4]> = SmallVec::new();
        if self.effect_orders.is_empty() {
            return nets;
        }

        let mut slots = self.effect_orders.clone();
        slots.sort_unstable();
        slots.dedup();

        for change in &self.order_nodes {
            if slots.binary_search(&change.slot).is_err()
                || nets.iter().any(|(slot, _)| *slot == change.slot)
            {
                continue;
            }
            let before = match change.op {
                Op::Created(_) => None,
                Op::Removed(index) => self.removed_order(index).map(|record| record.node),
                Op::Wrote(node) => Some(node),
            };
            nets.push((change.slot, before));
        }

        nets
    }

    /// Returns the slot of every order node the commit removed, with the removing command.
    pub(crate) fn removed_order_slots(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.order_nodes
            .iter()
            .filter(|change| matches!(change.op, Op::Removed(_)))
            .map(|change| (change.slot, change.command))
    }

    /// Returns the entries `splice` removed.
    pub(crate) fn removed(&self, splice: &Splice) -> &[OrderEntry] {
        &self.removed_entries[splice.removed.indices()]
    }

    /// Empties the journal, keeping the capacity of its lists.
    pub(crate) fn clear(&mut self) {
        self.transforms.clear();
        self.scrolls.clear();
        self.clips.clear();
        self.effects.clear();
        self.orders.clear();
        self.order_nodes.clear();
        self.removed_orders.clear();
        self.effect_orders.clear();
        self.removed_entries.clear();
        self.doubled.clear();
        self.unplaced.clear();
        self.freed.clear();
        self.paints.clear();
        self.params.clear();
        self.images.clear();
        self.textures.clear();
        self.shaders.clear();
        self.chunks.clear();
        self.replaced.clear();
        self.roots.clear();
        self.deferred.image_writes.clear();
        self.deferred.texture_frames.clear();
        self.declared.clear();
        self.rested.clear();
        self.tier = TierStage::default();
    }
}

/// Notes that the sequence of order node `parent` holds `entry` once more, and reports whether
/// sequences now hold it more than once.
pub(crate) fn place(scene: &mut Scene, entry: OrderEntry, parent: u32) -> bool {
    match entry {
        OrderEntry::Chunk(key) => scene.placements.place(key, parent),
        OrderEntry::Node(id) => scene.node_placements.place(id.slot(), parent),
    }
}

/// Notes that the sequence of order node `parent` holds `entry` once less.
pub(crate) fn unplace(scene: &mut Scene, entry: OrderEntry, parent: u32) {
    match entry {
        OrderEntry::Chunk(key) => scene.placements.unplace(key, parent),
        OrderEntry::Node(id) => scene.node_placements.unplace(id.slot(), parent),
    }
}
