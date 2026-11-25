//! The outputs of a compositor and what each one's renderer holds.

pub(crate) mod declared;
pub(crate) mod history;
pub(crate) mod pins;
pub(crate) mod renderer;
pub(crate) mod scroll;

use iceblink::OutputConfig;
use iceblink::damage::DamageSet;
use iceblink::geom::{Point, Rect, Size};
use iceblink::smallvec::SmallVec;
use iceblink::{ClientId, DeviceRect, FrameId, OrderId, RenderCapabilities, Shift, SubpixelOrder};

use crate::plan::Planner;

use self::declared::DeclaredCheck;
use self::history::PresentHistory;
use self::pins::DrawnPins;
use self::renderer::RendererState;
use self::scroll::PendingScroll;

/// One output: its configuration, its root and the damage owed to its next frame.
#[derive(Debug)]
pub(crate) struct Output {
    pub(crate) config: OutputConfig,
    /// The order node the output draws, and the client that owns it.
    pub(crate) root: Option<(ClientId, OrderId)>,
    /// Damage derived since the last frame, cut to the output.
    pub(crate) pending: DamageSet,
    /// The part of `pending` that commits of the compositor's own derived.
    pub(crate) driven: DamageSet,
    /// Whether a commit of the compositor's own is deriving its damage.
    pub(crate) driving: bool,
    /// Scroll offset changes since the last frame, which a shift may answer.
    pub(crate) scrolls: Vec<PendingScroll>,
    pub(crate) renderer: RendererState,
    pub(crate) history: PresentHistory,
    /// The frame built and not yet reported through `frame_done`.
    pub(crate) flight: Option<InFlight>,
    /// The id the next frame gets.
    pub(crate) next_frame: u64,
    /// The capabilities and subpixel order the last frame was built for.
    pub(crate) last_request: Option<(RenderCapabilities, Option<SubpixelOrder>)>,
    /// The present time of the last frame built.
    pub(crate) last_present_ns: Option<u64>,
    /// The build serial of the last frame built.
    pub(crate) built: u64,
    /// The subpixel order, scale bits and content generation the last frame lowered glyph runs
    /// with.
    pub(crate) text: Option<(Option<SubpixelOrder>, u32, u64)>,
    /// The planner of the output's frames, which keeps its buffers between frames.
    pub(crate) planner: Planner,
    /// The tiles the last frame drew, for a renderer without resident chunks.
    pub(crate) pins: DrawnPins,
    /// Where derived damage goes:
    ///
    /// - If no commit is declaring the output's damage (`None`), it is added to `pending`.
    /// - If a commit is declaring it and the damage is checked (`Some(Some(check))`), it is
    ///   checked against the declaration.
    /// - Otherwise (`Some(None)`) it is dropped.
    pub(crate) diverted: Option<Option<DeclaredCheck>>,
    /// When the output next changes without a call, as of its last build.
    pub(crate) wake: Option<u64>,
    /// The refresh interval of the last frame built, zero before the first.
    pub(crate) interval_ns: u64,
}

/// What a built frame owes if its outcome keeps damage.
#[derive(Debug)]
pub(crate) struct InFlight {
    pub(crate) id: FrameId,
    pub(crate) damage: DamageSet,
    pub(crate) shifts: SmallVec<[Shift; 1]>,
    pub(crate) present: DamageSet,
}

impl Output {
    pub(crate) fn new(config: OutputConfig) -> Self {
        let mut pending = DamageSet::full();
        pending.set_bound(Some(bounds_of(config)));
        let mut driven = DamageSet::new();
        driven.set_bound(Some(bounds_of(config)));

        Self {
            config,
            root: None,
            pending,
            driven,
            driving: false,
            scrolls: Vec::new(),
            renderer: RendererState::fresh(),
            history: PresentHistory::default(),
            flight: None,
            next_frame: 1,
            last_request: None,
            last_present_ns: None,
            built: 0,
            text: None,
            planner: Planner::new(),
            pins: DrawnPins::default(),
            diverted: None,
            wake: None,
            interval_ns: 0,
        }
    }

    /// Returns the output's pixels as a device rectangle.
    pub(crate) fn bounds(&self) -> DeviceRect {
        bounds_of(self.config)
    }

    /// Records that `frame` was built with damage `own` at build serial `serial`, and starts the
    /// next frame's damage empty.
    pub(crate) fn sent(&mut self, frame: &iceblink::Frame, own: DamageSet, serial: u64) {
        self.renderer.fresh = false;
        self.pending = DamageSet::new();
        self.pending.set_bound(Some(self.bounds()));
        self.driven = DamageSet::new();
        self.driven.set_bound(Some(self.bounds()));

        self.flight = Some(InFlight {
            id: frame.id,
            damage: own,
            shifts: frame.shifts.clone(),
            present: frame.damage.present,
        });
        self.built = serial;
    }

    /// Adds `rect` to the damage of the next frame.
    pub(crate) fn damage(&mut self, rect: DeviceRect) {
        if self.driving {
            self.driven.absorb(rect);
        }

        match &mut self.diverted {
            None => self.pending.absorb(rect),
            Some(Some(check)) => check.absorb(rect),
            Some(None) => {}
        }
    }

    /// Damages the whole output on the next frame.
    pub(crate) fn damage_all(&mut self) {
        if self.driving {
            self.driven.set_full();
        }

        match &mut self.diverted {
            None => self.pending.set_full(),
            Some(Some(check)) => check.set_full(),
            Some(None) => {}
        }
    }
}

/// Returns the device rectangle an output of `config` covers.
pub(crate) fn bounds_of(config: OutputConfig) -> DeviceRect {
    let [width, height] = config.size;
    Rect::new(
        Point::new(0, 0),
        Size::new(clamp_to_i32(width), clamp_to_i32(height)),
    )
}

fn clamp_to_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

/// The outputs of a compositor, by id.
pub(crate) type Outputs = rustc_hash::FxHashMap<iceblink::OutputId, Output>;
