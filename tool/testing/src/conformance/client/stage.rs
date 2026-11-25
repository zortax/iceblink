//! A stepped driver with one client and one output, whose frames a recorder logs.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use iceblink::{
    Anchor, BorderStyle, ChunkFlags, ChunkKey, ChunkProps, ClientId, ClipRef, Color, FrameClock,
    GesturePhase, Matrix4, OrderEntry, OrderId, OrderNode, OutputId, Paint, PaintKind, Quad, ROUND,
    RenderCapabilities, ScrollBehaviour, ScrollGesture, ScrollId, ScrollNode, ScrollPhysics,
    ScrollRouting, ScrollSource, SnapKind, Sources, SpaceRef, Transaction, TransformId,
    TransformNode,
};
use iceblink::{ClientEvent, CompositorClient, OutputConfig};

use crate::conformance::client::Rig;
use crate::conformance::client::flaky::Flaky;
use crate::conformance::client::recorder::{Log, Recorder, SharedLog, read};
use crate::validate::describe;

/// The refresh interval of the step clocks, in nanoseconds.
pub const STEP_NS: u64 = 16_666_667;

/// The capabilities of the recorder's renderer, with every fast path.
const CAPS: RenderCapabilities = RenderCapabilities {
    max_texture_size: 8192,
    subpixel_text: false,
    custom_shaders: true,
    vector_compute: false,
    can_shift: true,
    resident_chunks: true,
    blend_modes: 1,
    kept_rasters: false,
};

/// A stepped driver with one client, which counts its wakes, and one opaque 200 by 200 output
/// whose root order node draws under an identity transform.
pub struct ClientStage {
    /// The driver under test.
    pub driver: Box<dyn CompositorClient>,
    /// The only client.
    pub client: ClientId,
    /// The only output.
    pub output: OutputId,
    /// The root order node of the output.
    pub root: OrderId,
    /// The identity transform node at the root.
    pub space: TransformId,
    /// The frames the output's renderer drew.
    pub log: SharedLog,
    /// How often the client was woken.
    pub wakes: Arc<AtomicUsize>,
    /// The number of the next step.
    steps: u64,
    entries: u32,
}

impl ClientStage {
    /// Returns a stage past its first frame, built at the first step.
    pub fn new(rig: &Rig<'_>) -> Self {
        Self::drawn_by(rig, None)
    }

    /// Returns a stage whose renderer draws through a buffer-age presenter of one image, whose
    /// next acquisition times out while `fail` holds.
    pub fn flaky(rig: &Rig<'_>, fail: Arc<AtomicBool>) -> Self {
        Self::drawn_by(rig, Some(fail))
    }

    fn drawn_by(rig: &Rig<'_>, fail: Option<Arc<AtomicBool>>) -> Self {
        let mut driver = (rig.client)((rig.compositor)());
        let (client, _) = driver.register_client();

        let wakes = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&wakes);
        driver.set_waker(
            client,
            Some(Arc::new(move || {
                counted.fetch_add(1, Ordering::Relaxed);
            })),
        );

        let log = SharedLog::default();
        let output = recorded_output(&mut *driver, &log, fail);
        let mut tx = driver.transaction(client);
        let space = tx.create_transform(identity(None));
        let root = tx.create_order(OrderNode::default());
        tx.set_root(output, Some(root));
        driver.commit(tx);

        let mut stage = Self {
            driver,
            client,
            output,
            root,
            space,
            log,
            wakes,
            steps: 0,
            entries: 0,
        };
        stage.step();
        stage.events();
        stage
    }

    /// Returns a transaction of the stage's client.
    pub fn tx(&mut self) -> Transaction {
        self.driver.transaction(self.client)
    }

    /// Appends an entry to the root's sequence in `tx`.
    pub fn push(&mut self, tx: &mut Transaction, entry: OrderEntry) {
        tx.splice(self.root, self.entries, 0, &[entry]);
        self.entries += 1;
    }

    /// Returns the clock of the next step.
    pub fn clock(&self) -> FrameClock {
        step_clock(self.steps)
    }

    /// Steps the driver at the next step clock and waits until it applied the step.
    pub fn step(&mut self) {
        let clock = self.clock();
        self.advance();
        self.driver.step(clock);
        self.driver.sync();
    }

    /// Moves the next step clock on by one interval.
    pub fn advance(&mut self) {
        self.steps += 1;
    }

    /// Steps the driver again at the clock of the last step, and waits until it applied the step.
    pub fn step_again(&mut self) {
        let clock = step_clock(self.steps.saturating_sub(1));
        self.driver.step(clock);
        self.driver.sync();
    }

    /// Steps the driver `count` times.
    pub fn steps(&mut self, count: usize) {
        for _ in 0..count {
            self.step();
        }
    }

    /// Drains the client's events.
    pub fn events(&mut self) -> Vec<ClientEvent> {
        let mut events = Vec::new();
        self.driver.drain_events(self.client, &mut events);
        events
    }

    /// Returns how often the client was woken.
    pub fn woken(&self) -> usize {
        self.wakes.load(Ordering::Relaxed)
    }

    /// Returns the log of drawn frames.
    ///
    /// # Panics
    ///
    /// Panics when a drawn frame broke the frame contract.
    pub fn log(&self) -> std::sync::MutexGuard<'_, Log> {
        let log = read(&self.log);
        assert!(
            log.violations.is_empty(),
            "a frame breaks the frame contract:\n{}",
            describe(&log.violations)
        );
        log
    }

    /// Returns how many frames the output's renderer drew.
    pub fn frames(&self) -> usize {
        self.log().draws.len()
    }

    /// Sends one gesture step at `position` moving by `delta`, stamped with the next step's
    /// clock, and predicts that the compositor routes it as `predicted`.
    ///
    /// A `Step` phase comes from a wheel and every other phase from a touchpad.
    pub fn gesture(
        &mut self,
        position: [f32; 2],
        delta: [f32; 2],
        phase: GesturePhase,
        predicted: ScrollRouting,
    ) {
        let source = match phase {
            GesturePhase::Step => ScrollSource::Wheel,
            _ => ScrollSource::Touchpad,
        };
        let gesture = ScrollGesture {
            position,
            delta,
            phase,
            source,
            time_ns: self.clock().now_ns,
        };
        self.driver
            .scroll_input(self.client, self.output, gesture, predicted);
    }

    /// Commits a port over the whole output onto content `height` tall in rows of 20, scrolling
    /// vertically with `physics`, and returns its scroll node.
    ///
    /// The list is drawn by one step, and the events of that step are dropped.
    pub fn list(&mut self, height: f32, physics: ScrollPhysics) -> ScrollId {
        let mut tx = self.tx();
        let node = tx.create_scroll(ScrollNode {
            space: self.space,
            port: [0.0, 0.0, 200.0, 200.0],
            content: [200.0, height],
            offset: [0.0; 2],
            behaviour: Some(ScrollBehaviour {
                physics,
                snap: Arc::from([]),
                snap_kind: SnapKind::Mandatory,
                bounce: Sources::NONE,
                user_scrollable: [false, true],
                chain: Sources::NONE,
            }),
        });
        let content = tx.create_transform(TransformNode {
            parent: Some(self.space),
            local: Matrix4::IDENTITY,
            scroll: Some(node),
            anchor: Anchor::Scrolls,
        });

        let rows: Vec<[f32; 4]> = (0..(height / 20.0) as usize)
            .map(|row| [0.0, row as f32 * 20.0, 200.0, 20.0])
            .collect();
        let key = ChunkKey::new(1, 0);
        quads(&mut tx, key, content, &rows);
        self.push(&mut tx, OrderEntry::Chunk(key));
        self.driver.commit(tx);

        self.step();
        self.events();
        node
    }

    /// Returns the routing of a gesture to `node` of the stage's client.
    pub fn to(&self, node: ScrollId) -> ScrollRouting {
        ScrollRouting::Handled {
            node,
            client: self.client,
        }
    }
}

/// Creates an opaque 200 by 200 output whose renderer logs its frames into `log`, and returns it.
///
/// With `fail`, the renderer draws through a buffer-age presenter of one image, whose next
/// acquisition times out while `fail` holds.
pub(crate) fn recorded_output(
    driver: &mut dyn CompositorClient,
    log: &SharedLog,
    fail: Option<Arc<AtomicBool>>,
) -> OutputId {
    let recorded = Arc::clone(log);
    driver.create_output(
        OutputConfig {
            size: [200, 200],
            scale: 1.0,
            opaque: true,
        },
        Box::new(move || {
            let recorder = recorder(&recorded);
            Ok(Box::new(match fail {
                Some(fail) => recorder.presenting(Flaky::new(fail)),
                None => recorder,
            }))
        }),
    )
}

/// Returns a renderer with every fast path that logs its frames into `log`.
pub(crate) fn recorder(log: &SharedLog) -> Recorder {
    Recorder::new(CAPS, Arc::clone(log))
}

/// Returns the clock of step `step`, which starts `step` intervals after zero.
pub(crate) fn step_clock(step: u64) -> FrameClock {
    FrameClock {
        now_ns: step * STEP_NS,
        present_ns: (step + 1) * STEP_NS,
        interval_ns: STEP_NS,
    }
}

/// Records chunk `key` in `space` with one opaque quad per rectangle.
pub(crate) fn quads(tx: &mut Transaction, key: ChunkKey, space: TransformId, rects: &[[f32; 4]]) {
    let paint = tx.create_paint(Paint::Solid(Color([0.2, 0.4, 0.8, 1.0])));
    let props = ChunkProps {
        space,
        clip: None,
        flags: ChunkFlags::default(),
    };
    let mut chunk = tx.record(key, props);
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    for &bounds in rects {
        chunk.quad(Quad {
            bounds,
            radii: [0.0; 8],
            shapes: ROUND,
            border: [0.0; 4],
            fill,
            stroke: iceblink::PaintRef::NONE,
            paint_origin: [0.0; 2],
            clip: ClipRef::INHERIT,
            space: SpaceRef::INHERIT,
            style: BorderStyle::SOLID,
            _pad: [0; 3],
        });
    }
}

fn identity(parent: Option<TransformId>) -> TransformNode {
    TransformNode {
        parent,
        local: Matrix4::IDENTITY,
        scroll: None,
        anchor: Anchor::Scrolls,
    }
}
