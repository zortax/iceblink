//! A compositor under test with one client, one output and a root order node, driven frame by
//! frame through a model of its renderer.

use std::cell::Cell;
use std::sync::Arc;

use iceblink::damage::DamageSet;
use iceblink::{
    BufferAge, Built, ChunkKey, ChunkProps, ClientId, CommitError, DeviceRect, Frame, FrameClock,
    FrameOutcome, FrameRequest, FrameStats, IdSpace, OrderEntry, OrderId, OrderNode, OutputId,
    RenderCapabilities, SkipReason, SubpixelOrder, Transaction, TransformId,
};
use iceblink::{CommitReport, Compositor, OutputConfig};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{device, identity, props, record_quads};
use crate::conformance::scene::replay::{Picture, Replay};
use crate::fakes::{FakeFontRegistry, FakeGlyphRaster};

/// The side of the output, in pixels.
pub(crate) const SIDE: u32 = 128;

/// The capabilities of a renderer with every fast path the scene suite exercises.
pub(crate) const FULL: RenderCapabilities = RenderCapabilities {
    max_texture_size: 4096,
    subpixel_text: false,
    custom_shaders: true,
    vector_compute: false,
    can_shift: true,
    resident_chunks: true,
    blend_modes: (1 << 17) - 1,
    kept_rasters: false,
};

/// A compositor with one client, one opaque output of `SIDE` by `SIDE` pixels, a root order node
/// and an identity root transform, past its first frame.
pub(crate) struct Stage {
    pub(crate) compositor: Box<dyn Compositor>,
    pub(crate) client: ClientId,
    pub(crate) ids: IdSpace,
    pub(crate) output: OutputId,
    pub(crate) root: OrderId,
    pub(crate) space: TransformId,
    pub(crate) frame: Frame,
    pub(crate) caps: RenderCapabilities,
    /// The stripe order frames are requested with, when they may draw subpixel sprites.
    pub(crate) subpixel: Option<SubpixelOrder>,
    /// The picture the last frame drew, or an empty one before the first.
    pub(crate) picture: Picture,
    replay: Replay,
    /// Whether the renderer's kept target holds the last frame.
    kept: bool,
    /// How many entries the root's sequence holds.
    entries: u32,
    /// Whether a skip of the precision checks was reported.
    imprecise_reported: Cell<bool>,
}

impl Stage {
    /// Returns a stage over a compositor from `make`, drawn for a renderer with `caps`.
    pub(crate) fn new(make: MakeCompositor<'_>, caps: RenderCapabilities) -> Self {
        let mut compositor = make();
        compositor.use_font_registry(Arc::new(FakeFontRegistry::new()));
        compositor.use_glyph_raster(Arc::new(FakeGlyphRaster::new()));
        let (client, ids) = compositor.register_client();
        let output = compositor.create_output(OutputConfig {
            size: [SIDE, SIDE],
            scale: 1.0,
            opaque: true,
        });

        let mut tx = compositor.transaction(client);
        let space = tx.create_transform(identity(None));
        let root = tx.create_order(OrderNode::default());
        tx.set_root(output, Some(root));
        compositor.commit(tx).expect("the setup commits");

        let mut stage = Self {
            compositor,
            client,
            ids,
            output,
            root,
            space,
            frame: Frame::default(),
            caps,
            subpixel: None,
            picture: Picture::default(),
            replay: Replay::new(caps),
            kept: false,
            entries: 0,
            imprecise_reported: Cell::new(false),
        };
        stage.draw();
        stage
    }

    /// Returns a stage for a renderer with [`FULL`] capabilities.
    pub(crate) fn full(make: MakeCompositor<'_>) -> Self {
        Self::new(make, FULL)
    }

    /// Returns a transaction of the stage's client.
    pub(crate) fn tx(&mut self) -> Transaction {
        self.compositor.transaction(self.client)
    }

    /// Commits `tx`.
    ///
    /// # Panics
    ///
    /// Panics when the commit is rejected.
    #[track_caller]
    pub(crate) fn commit(&mut self, tx: Transaction) -> CommitReport {
        match self.compositor.commit(tx) {
            Ok(report) => report,
            Err(error) => panic!("a valid transaction was rejected: {error}"),
        }
    }

    /// Commits `tx` and returns the result.
    pub(crate) fn try_commit(&mut self, tx: Transaction) -> Result<CommitReport, CommitError> {
        self.compositor.commit(tx)
    }

    /// Appends `entry` to the root's sequence in `tx`.
    pub(crate) fn append(&mut self, tx: &mut Transaction, entry: OrderEntry) {
        tx.splice(self.root, self.entries, 0, &[entry]);
        self.entries += 1;
    }

    /// Records chunk `key` in the root space with one quad per `(bounds, colour)` and appends it
    /// to the root.
    pub(crate) fn add_quads(&mut self, key: ChunkKey, quads: &[([f32; 4], [u8; 4])]) {
        let mut tx = self.tx();
        record_quads(&mut tx, key, self.props(), quads);
        self.append(&mut tx, OrderEntry::Chunk(key));
        self.commit(tx);
    }

    /// Returns chunk props in the root space with no clip.
    pub(crate) fn props(&self) -> ChunkProps {
        props(self.space)
    }

    /// Builds a frame for the renderer's target and returns what building returned.
    ///
    /// A built frame is checked against the frame contract and applied to the renderer model,
    /// and [`Stage::picture`] then holds what it draws.
    #[track_caller]
    pub(crate) fn build(&mut self) -> Built {
        self.build_aged(self.age())
    }

    /// Builds a frame for an image of `age`, as [`Stage::build`] does.
    #[track_caller]
    pub(crate) fn build_aged(&mut self, age: BufferAge) -> Built {
        self.build_with(age, FrameClock::default())
    }

    /// Builds a frame for the renderer's target at `clock`, as [`Stage::build`] does.
    #[track_caller]
    pub(crate) fn build_at(&mut self, clock: FrameClock) -> Built {
        self.build_with(self.age(), clock)
    }

    /// Returns the age of the renderer's target.
    fn age(&self) -> BufferAge {
        if self.kept {
            BufferAge::Kept
        } else {
            BufferAge::Unknown
        }
    }

    #[track_caller]
    fn build_with(&mut self, age: BufferAge, clock: FrameClock) -> Built {
        let request = FrameRequest {
            caps: &self.caps,
            age,
            clock,
            subpixel: self.subpixel,
        };
        let built = self
            .compositor
            .build_frame(self.output, &request, &mut self.frame);
        if built == Built::Frame {
            self.picture = self.replay.apply(&self.frame);
        }
        built
    }

    /// Reports `outcome` for the last frame built.
    pub(crate) fn done(&mut self, outcome: FrameOutcome) {
        match outcome {
            FrameOutcome::Recovered | FrameOutcome::Skipped(SkipReason::DeviceUnavailable) => {
                self.replay.device_lost();
                self.kept = false;
            }
            _ if outcome.retires_damage() => self.kept = true,
            _ => {}
        }
        let id = self.frame.id;
        self.compositor.frame_done(self.output, id, &outcome);
    }

    /// Builds a frame, reports it presented, and returns what building returned.
    #[track_caller]
    pub(crate) fn draw(&mut self) -> Built {
        let built = self.build();
        if built == Built::Frame {
            self.done(presented());
        }
        built
    }

    /// Forgets the compositor's caches, which damages the whole output, then builds and presents
    /// a frame and returns what it draws.
    ///
    /// # Panics
    ///
    /// Panics unless the frame redraws the whole output.
    #[track_caller]
    pub(crate) fn redraw(&mut self) -> &Picture {
        self.compositor.forget_caches();
        let built = self.build();
        assert_eq!(built, Built::Frame, "forgetting the caches builds a frame");
        let whole = device(0, 0, self.frame.size[0] as i32, self.frame.size[1] as i32);
        assert!(
            covers(&self.frame.damage.draw, whole),
            "forgetting the caches damages the whole output, but the frame redraws {:?}",
            self.frame.damage.draw
        );
        self.done(presented());
        &self.picture
    }

    /// Returns the draw damage of the last frame built.
    pub(crate) fn damage(&self) -> &DamageSet {
        &self.frame.damage.draw
    }

    /// Reports whether the compositor reports the precision profile.
    ///
    /// The first time a stage finds it does not, the precision checks of `case` are reported
    /// skipped.
    pub(crate) fn precise(&self, case: &str) -> bool {
        let precise = self.compositor.capabilities().precise;
        if !precise && !self.imprecise_reported.replace(true) {
            eprintln!(
                "skipped: scene case {case}: the precision checks, since the compositor is not precise"
            );
        }
        precise
    }
}

/// A presented outcome with no statistics.
pub(crate) fn presented() -> FrameOutcome {
    FrameOutcome::Presented(FrameStats::default())
}

/// Reports whether every pixel of `rect` lies in `damage`, whichever held rectangles cover it.
pub(crate) fn covers(damage: &DamageSet, rect: DeviceRect) -> bool {
    crate::validate::covered(damage, rect)
}

/// The pixels `damage` covers, with the whole output for a full set.
pub(crate) fn area(damage: &DamageSet) -> i64 {
    damage.area().unwrap_or(i64::from(SIDE) * i64::from(SIDE))
}
