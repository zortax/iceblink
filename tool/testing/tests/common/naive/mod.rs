//! Builders and a frame loop shared by the naive compositor's tests.

pub(crate) mod gpu;
pub(crate) mod scene;

use iceblink::{
    Anchor, BufferAge, Built, ChunkFlags, ChunkKey, ChunkProps, ClientId, ClipRef, Frame,
    FrameClock, FrameOutcome, FrameRequest, FrameStats, Matrix4, OrderEntry, OrderId, OrderNode,
    OutputId, PaintRef, Quad, ROUND, RenderCapabilities, SpaceRef, SubpixelOrder, Transaction,
    TransformId, TransformNode,
};
use iceblink::{Compositor, OutputConfig};
use iceblink_testing::FrameValidator;
use iceblink_testing::naive::NaiveCompositor;
use iceblink_testing::validate::describe;

/// A compositor with one client and one output whose root draws `root`'s entries, and a frame
/// loop that checks every frame against the frame contract.
pub(crate) struct Harness {
    pub(crate) compositor: NaiveCompositor,
    pub(crate) client: ClientId,
    pub(crate) output: OutputId,
    pub(crate) root: OrderId,
    pub(crate) space: TransformId,
    pub(crate) caps: RenderCapabilities,
    pub(crate) subpixel: Option<SubpixelOrder>,
    pub(crate) frame: Frame,
    validator: FrameValidator,
    now: u64,
    /// The number of entries appended to the root.
    appended: u32,
}

impl Harness {
    /// Returns a harness with a 64 by 48 output.
    pub(crate) fn new() -> Self {
        Self::with_size([64, 48])
    }

    /// Returns a harness with an opaque output of `size`, a root order node and a root transform
    /// node, whose frames are built for a minimal renderer.
    pub(crate) fn with_size(size: [u32; 2]) -> Self {
        let mut compositor = NaiveCompositor::new();
        let (client, _) = compositor.register_client();
        let output = compositor.create_output(OutputConfig {
            size,
            scale: 1.0,
            opaque: true,
        });

        let mut transaction = compositor.transaction(client);
        let root = transaction.create_order(OrderNode::default());
        let space = transaction.create_transform(root_node());
        transaction.set_root(output, Some(root));
        compositor.commit(transaction).expect("the setup commits");

        let caps = RenderCapabilities::MINIMAL;
        let mut validator = FrameValidator::new();
        validator.set_capabilities(Some(caps));
        Self {
            compositor,
            client,
            output,
            root,
            space,
            caps,
            subpixel: None,
            frame: Frame::default(),
            validator,
            now: 0,
            appended: 0,
        }
    }

    /// Returns a new transaction of the harness's client.
    pub(crate) fn transaction(&mut self) -> Transaction {
        self.compositor.transaction(self.client)
    }

    /// Commits `transaction`, panicking when it is rejected.
    pub(crate) fn commit(&mut self, transaction: Transaction) {
        self.compositor
            .commit(transaction)
            .expect("the transaction commits");
    }

    /// Props under the harness's root transform node.
    pub(crate) fn props(&self) -> ChunkProps {
        ChunkProps {
            space: self.space,
            clip: None,
            flags: ChunkFlags::default(),
        }
    }

    /// Records `quads` into the chunk `key` and appends it to the root's entries.
    pub(crate) fn add_quads(&mut self, key: ChunkKey, quads: &[Quad]) {
        let mut transaction = self.transaction();
        let props = self.props();
        let mut recorder = transaction.record(key, props);
        for quad in quads {
            recorder.quad(*quad);
        }
        self.append(&mut transaction, OrderEntry::Chunk(key));
        self.commit(transaction);
    }

    /// Splices `entry` onto the end of the root's entries.
    pub(crate) fn append(&mut self, transaction: &mut Transaction, entry: OrderEntry) {
        transaction.splice(self.root, self.appended, 0, &[entry]);
        self.appended += 1;
    }

    /// Builds the next frame for a kept target, checks it, and reports `outcome` for it.
    ///
    /// Returns whether a frame was built.
    pub(crate) fn frame_with(&mut self, outcome: FrameOutcome) -> bool {
        self.now += 16_000_000;
        let request = FrameRequest {
            caps: &self.caps,
            age: BufferAge::Kept,
            clock: FrameClock {
                now_ns: self.now,
                present_ns: self.now + 8_000_000,
                interval_ns: 16_000_000,
            },
            subpixel: self.subpixel,
        };

        let built = self
            .compositor
            .build_frame(self.output, &request, &mut self.frame);
        if built != Built::Frame {
            return false;
        }

        if let Err(violations) = self.validator.check(&self.frame) {
            panic!(
                "frame {} breaks the frame contract:\n{}",
                self.frame.id.0,
                describe(&violations)
            );
        }

        if matches!(outcome, FrameOutcome::Recovered) {
            self.validator.device_lost();
        }
        self.compositor
            .frame_done(self.output, self.frame.id, &outcome);
        true
    }

    /// Builds, checks and presents the next frame, and returns whether one was built.
    pub(crate) fn frame(&mut self) -> bool {
        self.frame_with(FrameOutcome::Presented(FrameStats::default()))
    }

    /// Sets the stripe order later frames are requested with.
    pub(crate) fn set_subpixel(&mut self, order: Option<SubpixelOrder>) {
        self.subpixel = order;
    }

    /// Sets the capabilities later frames are built for.
    pub(crate) fn set_caps(&mut self, caps: RenderCapabilities) {
        self.caps = caps;
        self.validator.set_capabilities(Some(caps));
    }
}

/// A root transform node with the identity matrix.
pub(crate) fn root_node() -> TransformNode {
    TransformNode {
        parent: None,
        local: Matrix4::IDENTITY,
        scroll: None,
        anchor: Anchor::Scrolls,
    }
}

/// A borderless quad filled with an inline colour.
pub(crate) fn quad(bounds: [f32; 4], rgba: [u8; 4]) -> Quad {
    Quad {
        bounds,
        radii: [0.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill: PaintRef::color(rgba),
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        style: Default::default(),
        _pad: [0; 3],
    }
}
