//! A renderer that records frames instead of drawing them.

mod copy;

use iceblink::Renderer;
use iceblink::{
    BufferAge, Frame, FrameOutcome, FrameStats, MemoryReport, RenderCapabilities, RenderTarget,
    SkipReason,
};

use crate::capture::mirror::Mirror;
use crate::capture::transcript::{self, Transcript};

pub use self::copy::copy_frame;

/// One frame a [`CaptureRenderer`] was given.
#[derive(Debug)]
pub struct Captured {
    /// A copy of the frame.
    pub frame: Frame,
    /// The whole frame as text.
    pub transcript: Transcript,
    /// What the frame draws, as text.
    pub drawing: Transcript,
}

/// A renderer that applies each frame's work to a [`Mirror`], records the frame and its
/// transcripts, and puts no pixels anywhere.
///
/// It keeps the last frame it was given, or every frame when built with
/// [`keeping_all`](Self::keeping_all). A frame is `Skipped(Unconfigured)` before the first
/// `configure` and `Presented` after it, with the vector pass count and the resident chunk count
/// as its only statistics. Its kept target holds the last frame once one was presented, and loses
/// it on a `configure` to another size.
#[derive(Debug)]
pub struct CaptureRenderer {
    caps: RenderCapabilities,
    target: Option<RenderTarget>,
    /// Whether the kept target holds the last presented frame.
    kept: bool,
    mirror: Mirror,
    keep_all: bool,
    captured: Vec<Captured>,
    frames: u64,
}

impl CaptureRenderer {
    /// Returns a renderer with the capabilities of `RenderCapabilities::MINIMAL`.
    pub fn new() -> Self {
        Self::with_capabilities(RenderCapabilities::MINIMAL)
    }

    /// Returns a renderer that reports `caps`.
    pub fn with_capabilities(caps: RenderCapabilities) -> Self {
        Self {
            caps,
            target: None,
            kept: false,
            mirror: Mirror::new(),
            keep_all: false,
            captured: Vec::new(),
            frames: 0,
        }
    }

    /// Returns the renderer keeping every frame it is given.
    #[must_use]
    pub fn keeping_all(mut self) -> Self {
        self.keep_all = true;
        self
    }

    /// Returns the last frame given, or `None` before the first.
    pub fn last(&self) -> Option<&Captured> {
        self.captured.last()
    }

    /// Returns the transcript of the last frame given.
    pub fn transcript(&self) -> Option<&Transcript> {
        self.last().map(|captured| &captured.transcript)
    }

    /// Returns the frames kept, oldest first.
    pub fn captured(&self) -> &[Captured] {
        &self.captured
    }

    /// Removes and returns the frames kept.
    pub fn take(&mut self) -> Vec<Captured> {
        std::mem::take(&mut self.captured)
    }

    /// Returns how many frames the renderer was given.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Returns what the renderer holds after the last frame's work.
    pub fn mirror(&self) -> &Mirror {
        &self.mirror
    }

    /// Forgets everything held, as a lost device does.
    ///
    /// The next frame the compositor builds must then be built as the first for a new renderer.
    pub fn lose_device(&mut self) {
        self.mirror.reset();
        self.kept = false;
    }
}

impl Default for CaptureRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for CaptureRenderer {
    fn capabilities(&self) -> RenderCapabilities {
        self.caps
    }

    fn configure(&mut self, target: RenderTarget) {
        if self.target.is_some_and(|held| held.size != target.size) {
            self.kept = false;
        }
        self.target = Some(target);
    }

    fn target(&self) -> Option<RenderTarget> {
        self.target
    }

    fn target_age(&self) -> BufferAge {
        if self.kept {
            BufferAge::Kept
        } else {
            BufferAge::Unknown
        }
    }

    fn draw(&mut self, frame: &Frame) -> FrameOutcome {
        self.frames += 1;
        self.mirror.apply(frame);

        let (transcript, drawing) = transcript::both(frame, &self.mirror);
        let captured = Captured {
            frame: copy_frame(frame),
            transcript,
            drawing,
        };
        if !self.keep_all {
            self.captured.clear();
        }
        self.captured.push(captured);

        if self.target.is_none() {
            return FrameOutcome::Skipped(SkipReason::Unconfigured);
        }

        self.kept = true;
        FrameOutcome::Presented(FrameStats {
            vector_passes: frame.vector_plan.passes.len() as u32,
            chunks_resident: self.mirror.resident.len() as u32,
            ..FrameStats::default()
        })
    }

    fn memory(&self) -> MemoryReport {
        MemoryReport::default()
    }

    fn as_any(&self) -> Option<&dyn core::any::Any> {
        Some(self)
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn core::any::Any> {
        Some(self)
    }
}
