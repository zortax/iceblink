//! The retained state of one client: chunks, nodes, slots, resources and the order tree.

pub(crate) mod chunk;
pub(crate) mod content;
pub(crate) mod follows;
pub(crate) mod inks;
pub(crate) mod kind;
pub(crate) mod motion;
pub(crate) mod names;
pub(crate) mod parents;
pub(crate) mod reaches;
pub(crate) mod record;
pub(crate) mod rows;
pub(crate) mod slab;
pub(crate) mod stored;
pub(crate) mod travelled;

use iceblink::{ChunkKey, ClientId, DeviceRect};
use rustc_hash::{FxHashMap, FxHashSet};

use self::chunk::{ChunkDerived, ChunkRecord};
use self::inks::{ChunkInks, DeviceInks, NO_PIXELS, NodeInk, Still};
use self::kind::{Kind, ObjectRef};
use self::motion::Motion;
use self::parents::{ChunkParents, NodeParents};
use self::record::{
    ClipRecord, EffectRecord, ImageRecord, Links, OrderRecord, PaintRecord, ParamsRecord,
    ScrollRecord, ShaderRecord, TextureRecord, TransformRecord,
};
use self::rows::Rows;
use self::slab::Slab;
use crate::derive::chunks::{ItemInks, WalkedLists};
use crate::derive::order::SpliceOp;
use crate::derive::route::RouteCache;
use crate::geometry::coverage::Partial;
use crate::tier::Tier;

/// The objects a client has created, and the state derived from them.
#[derive(Debug)]
pub(crate) struct Scene {
    pub(crate) client: ClientId,
    pub(crate) transforms: Slab<TransformRecord>,
    pub(crate) scrolls: Slab<ScrollRecord>,
    pub(crate) clips: Slab<ClipRecord>,
    pub(crate) effects: Slab<EffectRecord>,
    pub(crate) orders: Slab<OrderRecord>,
    pub(crate) paints: Slab<PaintRecord>,
    pub(crate) params: Slab<ParamsRecord>,
    pub(crate) images: Slab<ImageRecord>,
    pub(crate) textures: Slab<TextureRecord>,
    pub(crate) shaders: Slab<ShaderRecord>,
    pub(crate) chunks: FxHashMap<ChunkKey, ChunkRecord>,
    /// The bytes the chunk records occupy, as last counted.
    pub(crate) chunk_bytes: u64,
    /// The routes of order nodes found so far.
    pub(crate) routes: RouteCache,
    /// The lists chunk refreshes measure item inks into.
    pub(crate) item_inks: ItemInks,
    /// The lists chunk refreshes find moved chunks into.
    pub(crate) walked: WalkedLists,
    /// The order nodes whose sequences hold each chunk key, once per entry.
    pub(crate) placements: ChunkParents,
    /// The order nodes whose sequences hold each order node, by slot, once per entry.
    pub(crate) node_placements: NodeParents,
    /// The chunks that draw with a shader reading the clock.
    pub(crate) timed: FxHashSet<ChunkKey>,
    /// The chunks holding a backdrop that reads outside what it writes.
    pub(crate) reading: FxHashSet<ChunkKey>,
    /// The order nodes whose group has a filter that reads outside what it writes.
    pub(crate) reading_groups: FxHashSet<u32>,
    /// Whether some clip node keeps the reaches of its users.
    pub(crate) reaches_kept: bool,
    /// The effect nodes with a shader filter.
    pub(crate) shaded_effects: FxHashSet<u32>,
    /// The compositor-driven state: animations, scroll motion, and the base values they
    /// override.
    pub(crate) tier: Tier,
    /// The transform nodes whose slight scale writes keep changing.
    pub(crate) motion: Motion,
    /// Emptied rows that order node sums reuse.
    pub(crate) spare_rows: Vec<Rows>,
    /// The splices of the order nodes that a commit changed by splices alone, while their inks
    /// are refreshed, sorted by node and in the order they applied.
    pub(crate) splices: Vec<SpliceOp>,
    /// How many refreshes of order node inks have run.
    pub(crate) order_refreshes: u32,
    /// Emptied derived states that new chunk records reuse.
    pub(crate) spare_derived: Vec<ChunkDerived>,
}

impl Scene {
    pub(crate) fn new(client: ClientId) -> Self {
        Self {
            client,
            transforms: Slab::default(),
            scrolls: Slab::default(),
            clips: Slab::default(),
            effects: Slab::default(),
            orders: Slab::default(),
            paints: Slab::default(),
            params: Slab::default(),
            images: Slab::default(),
            textures: Slab::default(),
            shaders: Slab::default(),
            chunks: FxHashMap::default(),
            chunk_bytes: 0,
            routes: RouteCache::default(),
            item_inks: ItemInks::default(),
            walked: WalkedLists::default(),
            placements: ChunkParents::default(),
            node_placements: NodeParents::default(),
            timed: FxHashSet::default(),
            reading: FxHashSet::default(),
            reading_groups: FxHashSet::default(),
            reaches_kept: false,
            shaded_effects: FxHashSet::default(),
            tier: Tier::default(),
            motion: Motion::default(),
            spare_rows: Vec::new(),
            splices: Vec::new(),
            order_refreshes: 0,
            spare_derived: Vec::new(),
        }
    }

    /// Counts the bytes of chunk `key` again, after its recording or its lowerings changed.
    pub(crate) fn recount(&mut self, key: ChunkKey) {
        if let Some(record) = self.chunks.get_mut(&key) {
            recount(&mut self.chunk_bytes, record);
        }
    }

    /// Counts the bytes of every chunk again.
    pub(crate) fn recount_all(&mut self) {
        for record in self.chunks.values_mut() {
            recount(&mut self.chunk_bytes, record);
        }
    }

    /// Stops counting the bytes of `record`, which left the chunk store.
    pub(crate) fn uncount(&mut self, record: &ChunkRecord) {
        self.chunk_bytes -= record.derived.counted;
    }

    /// Returns how far the content of scroll node `scroll` has moved on the device, or zero for a
    /// slot without a scroll node.
    pub(crate) fn travel(&self, scroll: u32) -> [i32; 2] {
        self.scrolls
            .at(scroll)
            .map_or([0; 2], |record| record.travel)
    }

    /// Returns a chunk's inks on the device.
    #[inline]
    pub(crate) fn device_inks<'a>(&self, inks: &'a ChunkInks) -> DeviceInks<'a> {
        match inks.carry() {
            Some(carry) => inks.at(self.travel(carry.scroll), self.still_pixels(carry.still)),
            None => inks.at([0; 2], NO_PIXELS),
        }
    }

    /// Returns an order node's ink on the device.
    #[inline]
    pub(crate) fn node_ink(&self, ink: &NodeInk) -> Option<DeviceRect> {
        match ink.carry() {
            Some(carry) => ink.at(self.travel(carry.scroll), self.still_pixels(carry.still)),
            None => ink.at([0; 2], NO_PIXELS),
        }
    }

    /// Returns the device pixels carried content is cut to.
    ///
    /// The still pixels of a clip node's chain whose edges are not whole are taken rounded out.
    #[inline]
    pub(crate) fn still_pixels(&self, still: Still) -> DeviceRect {
        match still {
            Still::Node(slot) => self
                .clips
                .at(slot)
                .map_or(NO_PIXELS, |record| record.still_pixels()),
            Still::Fixed { pixels, .. } => pixels,
        }
    }

    /// Returns where the clip links that cut carried content to its still pixels cover in part,
    /// or `None` when that cannot be bounded.
    pub(crate) fn still_partial(&self, still: Still) -> Option<Partial> {
        match still {
            Still::Node(slot) => self.clips.at(slot).and_then(|record| record.split.partial),
            Still::Fixed { partial, .. } => partial,
        }
    }

    /// Reports whether `object` refers to a live object.
    pub(crate) fn is_live(&self, object: ObjectRef) -> bool {
        self.links(object).is_some()
    }

    /// Returns the bookkeeping of the live object `object` refers to, if there is one.
    pub(crate) fn links(&self, object: ObjectRef) -> Option<&Links> {
        let slot = object.slot;
        match object.kind {
            Kind::Transform => self.transforms.at(slot).map(|r| &r.links),
            Kind::Scroll => self.scrolls.at(slot).map(|r| &r.links),
            Kind::Clip => self.clips.at(slot).map(|r| &r.links),
            Kind::Effect => self.effects.at(slot).map(|r| &r.links),
            Kind::Paint => self.paints.at(slot).map(|r| &r.links),
            Kind::Params => self.params.at(slot).map(|r| &r.links),
            Kind::Image => self.images.at(slot).map(|r| &r.links),
            Kind::Texture => self.textures.at(slot).map(|r| &r.links),
            Kind::Shader => self.shaders.at(slot).map(|r| &r.links),
        }
    }

    /// Returns the bookkeeping of the live object `object` refers to mutably, if there is one.
    pub(crate) fn links_mut(&mut self, object: ObjectRef) -> Option<&mut Links> {
        let slot = object.slot;
        match object.kind {
            Kind::Transform => self.transforms.at_mut(slot).map(|r| &mut r.links),
            Kind::Scroll => self.scrolls.at_mut(slot).map(|r| &mut r.links),
            Kind::Clip => self.clips.at_mut(slot).map(|r| &mut r.links),
            Kind::Effect => self.effects.at_mut(slot).map(|r| &mut r.links),
            Kind::Paint => self.paints.at_mut(slot).map(|r| &mut r.links),
            Kind::Params => self.params.at_mut(slot).map(|r| &mut r.links),
            Kind::Image => self.images.at_mut(slot).map(|r| &mut r.links),
            Kind::Texture => self.textures.at_mut(slot).map(|r| &mut r.links),
            Kind::Shader => self.shaders.at_mut(slot).map(|r| &mut r.links),
        }
    }

    /// Reports whether the slot of `object` holds a live object of `generation`.
    pub(crate) fn holds(&self, object: ObjectRef, generation: u32) -> bool {
        let slot = object.slot;
        match object.kind {
            Kind::Transform => self.transforms.get(slot, generation).is_some(),
            Kind::Scroll => self.scrolls.get(slot, generation).is_some(),
            Kind::Clip => self.clips.get(slot, generation).is_some(),
            Kind::Effect => self.effects.get(slot, generation).is_some(),
            Kind::Paint => self.paints.get(slot, generation).is_some(),
            Kind::Params => self.params.get(slot, generation).is_some(),
            Kind::Image => self.images.get(slot, generation).is_some(),
            Kind::Texture => self.textures.get(slot, generation).is_some(),
            Kind::Shader => self.shaders.get(slot, generation).is_some(),
        }
    }
}

/// Counts the bytes of `record` again into `total`, the bytes of the chunk store.
pub(crate) fn recount(total: &mut u64, record: &mut ChunkRecord) {
    let bytes = crate::frame::lower::lowered_bytes(record);

    *total = *total - record.derived.counted + bytes;
    record.derived.counted = bytes;
}
