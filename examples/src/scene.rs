//! The compositor state a demo records into.

use std::sync::Arc;

use iceblink::{
    Anchor, ChunkFlags, ChunkKey, ChunkProps, ClientId, Matrix4, OrderEntry, OrderId, OrderNode,
    OutputId, Transaction, TransformId, TransformNode,
};
use iceblink::{
    ClientEvent, Compositor as _, CompositorClient as _, OutputConfig, RendererFactory,
};
use iceblink_core::CompositorImpl;
use iceblink_driver::{DriverOptions, InProcess};
use iceblink_testing::fonts::{LATIN, read_font};
use iceblink_text::SwashRaster;
use iceblink_text::typeset::FontRegistry;

use crate::text::Font;

/// A compositor client with a root order node, a root transform in logical pixels, and the demo
/// font.
///
/// The compositor runs on the window's thread, behind an in-process driver.
pub struct Scene {
    pub(crate) driver: InProcess,
    pub(crate) client: ClientId,
    pub(crate) root: OrderId,
    pub(crate) space: TransformId,
    pub(crate) font: Font,
    /// How many entries the root's sequence holds.
    pub(crate) entries: u32,
    fonts: Option<Arc<FontRegistry>>,
}

impl Scene {
    /// Returns a new transaction of the demo's client.
    pub fn transaction(&mut self) -> Transaction {
        self.driver.transaction(self.client)
    }

    /// The transform node that maps logical pixels to device pixels.
    pub fn space(&self) -> TransformId {
        self.space
    }

    /// The font text is set in.
    pub fn font(&self) -> &Font {
        &self.font
    }

    /// The registry the demo's [`fonts`](crate::Demo::fonts) gave, which the compositor shares.
    pub fn fonts(&self) -> Option<&Arc<FontRegistry>> {
        self.fonts.as_ref()
    }

    /// Appends chunk `key` to the root's sequence, so it is drawn over every chunk appended before.
    pub fn append(&mut self, tx: &mut Transaction, key: ChunkKey) {
        tx.splice(self.root, self.entries, 0, &[OrderEntry::Chunk(key)]);
        self.entries += 1;
    }

    /// Returns chunk props in `space` with no clip.
    pub fn props(space: TransformId) -> ChunkProps {
        ChunkProps {
            space,
            clip: None,
            flags: ChunkFlags::default(),
        }
    }
}

impl Scene {
    /// Returns a compositor with one output of `config` drawn by `renderer`, a glyph rasteriser
    /// and the test font, and a client whose root transform scales logical pixels by the
    /// output's scale.
    ///
    /// The compositor shares `fonts` when given.
    pub(crate) fn open(
        config: OutputConfig,
        fonts: Option<Arc<FontRegistry>>,
        renderer: RendererFactory,
    ) -> (Self, OutputId) {
        let mut compositor = CompositorImpl::new();
        compositor.use_glyph_raster(Arc::new(SwashRaster::new()));
        if let Some(fonts) = &fonts {
            compositor.use_font_registry(Arc::clone(fonts) as _);
        }
        let data = read_font(LATIN);
        let id = compositor.font_registry().register(data.clone());
        let mut driver = InProcess::new(Box::new(compositor), DriverOptions::default());
        let (client, _) = driver.register_client();
        let output = driver.create_output(config, renderer);
        driver.watch_frames(client, output, true);

        let mut tx = driver.transaction(client);
        let space = tx.create_transform(TransformNode {
            parent: None,
            local: scaled(config.scale),
            scroll: None,
            anchor: Anchor::Scrolls,
        });
        let root = tx.create_order(OrderNode::default());
        tx.set_root(output, Some(root));

        let mut scene = Self {
            driver,
            client,
            root,
            space,
            font: Font { id, data },
            entries: 0,
            fonts,
        };
        scene.commit(tx);
        (scene, output)
    }

    /// Queues a commit of `tx`.
    pub(crate) fn commit(&mut self, tx: Transaction) {
        self.driver.commit(tx);
    }

    /// Moves the client's events into `out`.
    ///
    /// # Panics
    ///
    /// Panics if the compositor rejected a transaction.
    pub(crate) fn events(&mut self, out: &mut Vec<ClientEvent>) {
        self.driver.drain_events(self.client, out);
        for event in out.iter() {
            if let ClientEvent::Rejected { error, .. } = event {
                panic!("the demo's transaction is rejected: {error}");
            }
        }
    }
}

/// Returns the matrix that maps logical pixels to device pixels at `scale`.
pub(crate) fn scaled(scale: f32) -> Matrix4 {
    Matrix4::scale(scale, scale, 1.0)
}
