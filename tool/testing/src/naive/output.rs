//! Output state: the configuration, whether a frame is owed, and what the renderer holds.

use std::collections::HashSet;

use iceblink::OutputConfig;
use iceblink::{FrameId, FrameOutcome, RenderCapabilities, SkipReason, SubpixelOrder};

use crate::naive::content::HeldPages;
use crate::naive::frame::{OutputSlots, ShaderSlots};

/// The state the compositor keeps for one output.
pub(crate) struct Output {
    pub(crate) config: OutputConfig,
    /// Whether the next frame has damage.
    pub(crate) owes_frame: bool,
    /// The id of the last frame built.
    pub(crate) last_frame: u64,
    /// The frame built and not yet reported with `frame_done`.
    pub(crate) pending: Option<FrameId>,
    /// The pages the renderer holds.
    pub(crate) pages: HeldPages,
    /// The shaders the renderer holds.
    pub(crate) shaders: ShaderSlots,
    pub(crate) slots: OutputSlots,
    /// Whether the next frame drops every resident chunk of the renderer.
    pub(crate) reset_resident: bool,
    /// The capabilities and stripe order the last frame was built for.
    pub(crate) built_for: Option<(RenderCapabilities, Option<SubpixelOrder>)>,
    /// The present time of the last frame built.
    pub(crate) last_present: Option<u64>,
    /// The pages the last frame drew from, which eviction keeps.
    pub(crate) drawn_pages: HashSet<u32>,
    /// Whether the last frame drew with a shader that reads the clock.
    pub(crate) reads_time: bool,
}

impl Output {
    pub(crate) fn new(config: OutputConfig) -> Self {
        Self {
            config,
            owes_frame: true,
            last_frame: 0,
            pending: None,
            pages: HeldPages::new(),
            shaders: ShaderSlots::default(),
            slots: OutputSlots::new(),
            reset_resident: true,
            built_for: None,
            last_present: None,
            drawn_pages: HashSet::new(),
            reads_time: false,
        }
    }

    /// Treats the renderer as one that holds nothing, so the next frame is built as its first.
    pub(crate) fn forget_renderer(&mut self) {
        self.pages.clear();
        self.shaders.forget();
        self.reset_resident = true;
        self.owes_frame = true;
    }

    /// Applies the outcome of drawing `frame`.
    ///
    /// An outcome for a frame other than the pending one is ignored. Otherwise:
    ///
    /// - If the outcome keeps the frame's damage, a frame stays owed.
    /// - If the outcome is `Recovered` or `Skipped(DeviceUnavailable)`, the renderer is also
    ///   treated as one that holds nothing.
    pub(crate) fn done(&mut self, frame: FrameId, outcome: &FrameOutcome) {
        if self.pending != Some(frame) {
            return;
        }
        self.pending = None;

        if !outcome.retires_damage() {
            self.owes_frame = true;
        }

        if matches!(
            outcome,
            FrameOutcome::Recovered | FrameOutcome::Skipped(SkipReason::DeviceUnavailable)
        ) {
            self.forget_renderer();
        }
    }
}
