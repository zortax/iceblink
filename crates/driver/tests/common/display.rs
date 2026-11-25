//! A fake display that refreshes at a fixed period and reports when presents reached it, and a
//! driver of one output on it.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use iceblink::damage::DamageSet;
use iceblink::{
    Acquired, BorderStyle, BufferAge, ChunkFlags, ChunkKey, ChunkProps, ClipRef, Color, Frame,
    FrameOutcome, FrameStats, MemoryReport, OrderEntry, OrderNode, Paint, PaintKind, PaintRef,
    PresentCapabilities, PresentFeedback, PresentMode, PresentResult, Quad, ROUND,
    RenderCapabilities, RenderTarget, STALE_DAMAGE, SkipReason, SpaceRef, Transaction,
};
use iceblink::{CompositorClient, OutputConfig, Presenter, Renderer};
use iceblink_core::CompositorImpl;
use iceblink_driver::{DriverOptions, InProcess, now_ns};
use iceblink_testing::fakes::NullRenderer;

/// When the display reports the `index`th present it showed at `shown`, or `None` when it loses
/// the report.
pub(crate) type Report = fn(usize, u64) -> Option<u64>;

/// How long a frame that shows the commit of loop index `index` takes to draw, in nanoseconds.
/// The output's first frame shows the commit of index 0.
pub(crate) type Cost = fn(usize) -> u64;

/// What the display showed: each frame's planned display time and when it was shown.
pub(crate) type Shown = Arc<Mutex<Vec<(u64, u64)>>>;

/// When each present the display showed was made.
type Presented = Arc<Mutex<Vec<u64>>>;

/// How many chunks the client's commits record at most, side by side on the output.
const CHUNKS: u32 = 128;

/// A display that refreshes at every multiple of `period_ns` and takes an image only if it was
/// presented `takes_ns` before the refresh.
pub(crate) struct Display {
    period_ns: u64,
    takes_ns: u64,
    /// How long a frame takes to draw, by the index of the newest commit it shows.
    cost: Cost,
    /// The index of the newest commit of the run.
    committed: Arc<AtomicUsize>,
    /// The presents not reported yet: their numbers, when they reached the display, and when they
    /// are reported.
    pending: Vec<(u64, u64, Option<u64>)>,
    shown: Shown,
    presented: Presented,
    report: Report,
    /// The reports the client passes on, when the presenter reports nothing itself.
    by_client: Option<Reported>,
    /// How long after a frame reaches the display the platform lets the next one present, if it
    /// holds frames until then.
    gate_ns: Option<u64>,
}

/// The times presents reached the display, which the client reports to the driver.
pub(crate) type Reported = Arc<Mutex<Vec<u64>>>;

impl Display {
    /// Returns when an image presented now reaches the display.
    fn refresh_for(&self, now: u64) -> u64 {
        (now + self.takes_ns).div_ceil(self.period_ns) * self.period_ns
    }

    /// Reports whether the platform lets a frame present at `now`.
    fn open(&self, now: u64) -> bool {
        let last = self.shown.lock().unwrap().last().map(|&(_, shown)| shown);
        self.gate_ns
            .is_none_or(|gate| last.is_none_or(|shown| now >= shown + gate))
    }

    /// Presents a frame built for display time `planned` now.
    fn show(&mut self, planned: u64) {
        let now = now_ns();
        let shown = self.refresh_for(now);
        self.presented.lock().unwrap().push(now);
        let index = self.shown.lock().unwrap().len();
        let reported = (self.report)(index, shown);
        self.pending.push((index as u64 + 1, shown, reported));
        self.shown.lock().unwrap().push((planned, shown));
    }
}

/// A null renderer that presents to a [`Display`].
pub(crate) struct Screen {
    inner: NullRenderer,
    display: Display,
    /// The display time of the frame the platform holds, if any.
    held: Option<u64>,
}

impl Renderer for Screen {
    fn capabilities(&self) -> RenderCapabilities {
        self.inner.capabilities()
    }

    fn configure(&mut self, target: RenderTarget) {
        self.inner.configure(target);
    }

    fn target(&self) -> Option<RenderTarget> {
        self.inner.target()
    }

    fn target_age(&self) -> BufferAge {
        self.inner.target_age()
    }

    fn draw(&mut self, frame: &Frame) -> FrameOutcome {
        let committed = self.display.committed.load(Ordering::Relaxed);
        thread::sleep(Duration::from_nanos((self.display.cost)(committed)));
        let outcome = self.inner.draw(frame);
        if !matches!(outcome, FrameOutcome::Presented(_)) {
            return outcome;
        }
        if !self.display.open(now_ns()) {
            self.held = Some(frame.clock.present_ns);
            return FrameOutcome::Skipped(SkipReason::Held);
        }
        self.held = None;
        self.display.show(frame.clock.present_ns);
        outcome
    }

    fn present_held(&mut self) -> FrameOutcome {
        let Some(planned) = self.held else {
            return FrameOutcome::Skipped(SkipReason::Undamaged);
        };
        if !self.display.open(now_ns()) {
            return FrameOutcome::Skipped(SkipReason::Held);
        }
        self.held = None;
        self.display.show(planned);
        FrameOutcome::Presented(FrameStats::default())
    }

    fn presenter(&mut self) -> Option<&mut dyn Presenter> {
        Some(&mut self.display)
    }

    fn memory(&self) -> MemoryReport {
        self.inner.memory()
    }
}

static NO_DEBT: DamageSet<STALE_DAMAGE> = DamageSet::new();

impl Presenter for Display {
    fn capabilities(&self) -> PresentCapabilities {
        PresentCapabilities {
            mode: PresentMode::KeptTarget,
            partial_present: false,
            feedback: self.by_client.is_none(),
        }
    }

    fn configure(&mut self, _size: [u32; 2], _opaque: bool) -> Result<(), SkipReason> {
        Ok(())
    }

    fn acquire(&mut self) -> Acquired {
        Acquired::Skipped(SkipReason::Unacquired)
    }

    fn acquired(&self) -> Option<u32> {
        None
    }

    fn release(&mut self, _image: u32) {}

    fn composed(&mut self, _damage: &DamageSet) {}

    fn debt(&self, _image: u32) -> &DamageSet<STALE_DAMAGE> {
        &NO_DEBT
    }

    fn present(&mut self, _image: u32, _damage: &DamageSet) -> PresentResult {
        PresentResult::Presented
    }

    /// Reports as many presents as refreshes the display takes an image ahead, rounded up.
    fn display_depth(&self) -> u32 {
        u32::try_from(self.takes_ns.div_ceil(self.period_ns).max(1)).unwrap_or(u32::MAX)
    }

    fn presents(&self) -> u64 {
        self.shown.lock().unwrap().len() as u64
    }

    fn next_deadline(&self) -> Option<u64> {
        None
    }

    /// Reports the presents that reached the display by now, without the refresh interval.
    fn feedback(&mut self, out: &mut Vec<PresentFeedback>) {
        let now = now_ns();
        self.pending.retain(|&(present, shown, reported)| {
            if shown.max(reported.unwrap_or(0)) > now {
                return true;
            }
            if let (Some(reported), Some(by_client)) = (reported, &self.by_client) {
                by_client.lock().unwrap().push(reported);
            } else if let Some(reported) = reported {
                out.push(PresentFeedback {
                    image: 0,
                    present,
                    presented_ns: reported,
                    refresh_ns: 0,
                });
            }
            false
        });
    }
}

/// What the driver is told of the display.
pub(crate) struct Told {
    /// The display's refresh period.
    pub(crate) period_ns: u64,
    /// The interval the driver assumes.
    pub(crate) assumed_ns: u64,
    /// The interval the client reports, if any.
    pub(crate) known_ns: Option<u64>,
    /// Whether the client reports when presents reached the display, in place of the presenter.
    pub(crate) client_reports: bool,
    /// How long after a frame reaches the display the platform holds the next one, if it does.
    pub(crate) gate_ns: Option<u64>,
}

/// A driver of one output on a [`Display`], with the client's content so far.
pub(crate) struct Rig {
    pub(crate) driver: InProcess,
    client: iceblink::ClientId,
    output: iceblink::OutputId,
    root: iceblink::OrderId,
    space: iceblink::TransformId,
    paint: iceblink::PaintId,
    shown: Shown,
    presented: Presented,
    committed: Arc<AtomicUsize>,
    by_client: Option<Reported>,
}

impl Rig {
    /// Returns a driver told `told` of a display that takes images `takes_ns` before a refresh
    /// and reports its presents with `report`, drawing frames that take `cost`.
    pub(crate) fn new(told: &Told, cost: Cost, takes_ns: u64, report: Report) -> Self {
        let options = DriverOptions {
            interval_ns: told.assumed_ns,
            ..DriverOptions::default()
        };
        let mut driver = InProcess::new(Box::new(CompositorImpl::new()), options);
        let (client, _) = driver.register_client();
        let shown = Shown::default();
        let presented = Presented::default();
        let committed = Arc::new(AtomicUsize::new(0));
        let by_client = told.client_reports.then(Reported::default);
        let display = Display {
            period_ns: told.period_ns,
            takes_ns,
            cost,
            committed: Arc::clone(&committed),
            pending: Vec::new(),
            shown: Arc::clone(&shown),
            presented: Arc::clone(&presented),
            report,
            by_client: by_client.clone(),
            gate_ns: told.gate_ns,
        };
        // Wide enough that every frame's new chunk is on the output.
        let config = OutputConfig {
            size: [2 * CHUNKS + 8, 64],
            scale: 1.0,
            opaque: true,
        };
        let output = driver.create_output(
            config,
            Box::new(move || {
                Ok(Box::new(Screen {
                    inner: NullRenderer::new(),
                    display,
                    held: None,
                }))
            }),
        );
        driver.set_refresh(output, told.known_ns);
        let mut tx = driver.transaction(client);
        let root = tx.create_order(OrderNode::default());
        tx.set_root(output, Some(root));
        let space = tx.create_transform(iceblink::TransformNode {
            parent: None,
            local: iceblink::Matrix4::IDENTITY,
            scroll: None,
            anchor: iceblink::Anchor::Scrolls,
        });
        let paint = tx.create_paint(Paint::Solid(Color([0.2, 0.4, 0.8, 1.0])));
        driver.commit(tx);
        Self {
            driver,
            client,
            output,
            root,
            space,
            paint,
            shown,
            presented,
            committed,
            by_client,
        }
    }

    /// Commits a change for loop index `index`, and passes on the reports the client holds.
    pub(crate) fn commit(&mut self, index: u64) {
        let mut tx = self.driver.transaction(self.client);
        record(&mut tx, index, self.space, self.paint);
        tx.splice(
            self.root,
            index as u32,
            0,
            &[OrderEntry::Chunk(ChunkKey::new(index, 0))],
        );
        self.driver.commit(tx);
        if let Some(by_client) = &self.by_client {
            for presented_ns in by_client.lock().unwrap().drain(..) {
                self.driver.report_presented(self.output, presented_ns);
            }
        }
        self.committed.store(index as usize, Ordering::Relaxed);
    }

    /// Pumps the driver once its next frame latched, and returns whether a frame was due.
    pub(crate) fn pump_when_due(&mut self) -> bool {
        let Some(due) = self.driver.stats().next_due_ns else {
            return false;
        };
        thread::sleep(Duration::from_nanos(due.saturating_sub(now_ns())));
        self.driver.pump();
        true
    }

    /// Returns what the display showed so far.
    pub(crate) fn shown(&self) -> Vec<(u64, u64)> {
        self.shown.lock().unwrap().clone()
    }

    /// Returns when each present the display showed was made.
    pub(crate) fn presented(&self) -> Vec<u64> {
        self.presented.lock().unwrap().clone()
    }
}

/// Reports every present when it reached the display.
pub(crate) fn on_time(_: usize, shown: u64) -> Option<u64> {
    Some(shown)
}

fn record(
    tx: &mut Transaction,
    index: u64,
    space: iceblink::TransformId,
    paint: iceblink::PaintId,
) {
    let props = ChunkProps {
        space,
        clip: None,
        flags: ChunkFlags::default(),
    };
    let mut chunk = tx.record(ChunkKey::new(index, 0), props);
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(Quad {
        bounds: [index as f32 * 2.0, 0.0, 2.0, 2.0],
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
}
