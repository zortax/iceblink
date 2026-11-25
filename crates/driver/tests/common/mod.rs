//! A client's content on one output of a driver, shared by the driver tests.

// Every test binary compiles this module and uses a part of it.
#![allow(dead_code)]

pub(crate) mod display;
pub(crate) mod thread;

use iceblink::{
    Anchor, BorderStyle, ChunkFlags, ChunkKey, ChunkProps, ClientId, ClipRef, Color, Matrix4,
    OrderEntry, OrderId, OrderNode, OutputId, Paint, PaintId, PaintKind, PaintRef, Quad, ROUND,
    SpaceRef, Transaction, TransformId, TransformNode,
};
use iceblink::{CompositorClient, OutputConfig, RendererFactory};

/// A client of a driver with one output of 64 by 64 pixels.
///
/// The output's root, space and paint are committed with the first change, so the output's frames
/// start only then.
pub(crate) struct Scene {
    pub(crate) client: ClientId,
    pub(crate) output: OutputId,
    root: OrderId,
    space: TransformId,
    paint: PaintId,
    setup: Option<Transaction>,
    chunks: u32,
}

impl Scene {
    /// Registers a client on `driver` and creates its output drawn by `renderer`.
    pub(crate) fn new(driver: &mut dyn CompositorClient, renderer: RendererFactory) -> Self {
        let (client, _) = driver.register_client();
        let config = OutputConfig {
            size: [64, 64],
            scale: 1.0,
            opaque: true,
        };
        let output = driver.create_output(config, renderer);
        let mut tx = driver.transaction(client);
        let root = tx.create_order(OrderNode::default());
        tx.set_root(output, Some(root));
        let space = tx.create_transform(TransformNode {
            parent: None,
            local: Matrix4::IDENTITY,
            scroll: None,
            anchor: Anchor::Scrolls,
        });
        let paint = tx.create_paint(Paint::Solid(Color([0.2, 0.4, 0.8, 1.0])));
        Self {
            client,
            output,
            root,
            space,
            paint,
            setup: Some(tx),
            chunks: 0,
        }
    }

    /// Commits a new chunk with one quad, which changes what the output shows.
    pub(crate) fn change(&mut self, driver: &mut dyn CompositorClient) {
        let mut tx = self
            .setup
            .take()
            .unwrap_or_else(|| driver.transaction(self.client));
        let key = ChunkKey::new(u64::from(self.chunks) + 1, 0);
        let props = ChunkProps {
            space: self.space,
            clip: None,
            flags: ChunkFlags::default(),
        };
        let mut chunk = tx.record(key, props);
        let fill = chunk.paint_ref(PaintKind::SOLID, self.paint);
        chunk.quad(Quad {
            bounds: [(self.chunks % 32) as f32 * 2.0, 0.0, 2.0, 2.0],
            radii: [0.0; 8],
            shapes: ROUND,
            border: [0.0; 4],
            fill,
            stroke: PaintRef::NONE,
            paint_origin: [0.0; 2],
            clip: ClipRef::INHERIT,
            space: SpaceRef::INHERIT,
            style: BorderStyle::SOLID,
            _pad: [0; 3],
        });
        tx.splice(self.root, self.chunks, 0, &[OrderEntry::Chunk(key)]);
        self.chunks += 1;
        driver.commit(tx);
    }
}
