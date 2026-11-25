//! The retained scene: every client's nodes, slots, resources, chunks and order tree.

mod image;
mod table;

use std::collections::HashMap;
use std::sync::Arc;

use iceblink::{
    ChunkKey, ChunkProps, ClientId, ClipNode, EffectNode, IdSpace, ItemKind, OrderEntry, OrderId,
    OrderNode, OutputId, Paint, ParamsBlock, RecordedChunk, ScrollNode, ShaderDesc, TextureDesc,
    TextureId, TransformNode,
};

pub(crate) use self::image::StoredImage;
pub(crate) use self::table::Table;

/// Every client, and the order node each output draws.
#[derive(Default)]
pub(crate) struct Scene {
    pub(crate) clients: HashMap<ClientId, Client>,
    /// The root of each output that has one, with the client that owns it.
    pub(crate) roots: HashMap<OutputId, (ClientId, OrderId)>,
}

impl Scene {
    /// Reports whether chunk `key` of `client` holds a path item at `position`.
    pub(crate) fn holds_path(&self, (client, key, position): (ClientId, ChunkKey, usize)) -> bool {
        self.clients
            .get(&client)
            .and_then(|client| client.chunks.get(&key))
            .and_then(|chunk| chunk.recorded.ops.get(position))
            .is_some_and(|op| op.kind == ItemKind::PATH)
    }
}

/// The objects of one client.
#[derive(Clone)]
pub(crate) struct Client {
    pub(crate) ids: IdSpace,
    pub(crate) transforms: Table<TransformNode>,
    pub(crate) scrolls: Table<ScrollNode>,
    pub(crate) clips: Table<ClipNode>,
    pub(crate) effects: Table<EffectNode>,
    pub(crate) orders: Table<Order>,
    pub(crate) paints: Table<Paint>,
    pub(crate) params: Table<ParamsBlock>,
    pub(crate) images: Table<StoredImage>,
    pub(crate) textures: Table<(TextureId, TextureDesc)>,
    pub(crate) shaders: Table<Arc<ShaderDesc>>,
    pub(crate) chunks: HashMap<ChunkKey, Chunk>,
}

impl Client {
    pub(crate) fn new(ids: IdSpace) -> Self {
        Self {
            ids,
            transforms: Table::default(),
            scrolls: Table::default(),
            clips: Table::default(),
            effects: Table::default(),
            orders: Table::default(),
            paints: Table::default(),
            params: Table::default(),
            images: Table::default(),
            textures: Table::default(),
            shaders: Table::default(),
            chunks: HashMap::new(),
        }
    }
}

/// An order node and the entries it holds.
#[derive(Clone, Debug)]
pub(crate) struct Order {
    pub(crate) node: OrderNode,
    pub(crate) entries: Vec<OrderEntry>,
}

/// A retained chunk: its last recording, and the props and offset it is drawn with.
#[derive(Clone, Debug)]
pub(crate) struct Chunk {
    pub(crate) recorded: Arc<RecordedChunk>,
    /// The props of the recording, or of the last `set_props` since.
    pub(crate) props: ChunkProps,
    pub(crate) offset: [f32; 2],
}
