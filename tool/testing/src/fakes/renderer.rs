//! A renderer that draws nothing and records what it was given.

use std::collections::VecDeque;

use iceblink::Renderer;
use iceblink::{
    BufferAge, Frame, FrameDamage, FrameId, FrameOutcome, FrameStats, MemoryReport, OutputId,
    RenderCapabilities, RenderTarget, SkipReason,
};

use crate::validate::{FrameValidator, FrameViolation};

/// One frame a [`NullRenderer`] was asked to draw.
#[derive(Clone, Debug, PartialEq)]
pub struct Draw {
    /// The frame's id.
    pub frame: FrameId,
    /// The output the frame is for.
    pub output: OutputId,
    /// The frame's damage.
    pub damage: FrameDamage,
    /// How many shifts the frame carried.
    pub shifts: usize,
    /// What the renderer answered.
    pub outcome: FrameOutcome,
}

/// A renderer that draws nothing, answers each frame with a scripted outcome, and records every
/// frame with its answer.
///
/// Its kept target holds the last frame once a frame retired its damage, and loses it on a
/// `configure` to another size and on an outcome that rebuilds or loses the device. A frame is
/// answered as follows:
///
/// - If outcomes were pushed, the oldest one answers.
/// - Otherwise, if no target is configured, the frame is `Skipped(Unconfigured)`.
/// - Otherwise the frame is `Presented`, with its draw damage as `damage_px`.
#[derive(Debug)]
pub struct NullRenderer {
    caps: RenderCapabilities,
    target: Option<RenderTarget>,
    /// Whether the kept target holds the last composed frame.
    kept: bool,
    /// Whether a held frame waits for `present_held`.
    held: bool,
    script: VecDeque<FrameOutcome>,
    draws: Vec<Draw>,
    validator: Option<FrameValidator>,
    violations: Vec<FrameViolation>,
}

impl NullRenderer {
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
            held: false,
            script: VecDeque::new(),
            draws: Vec::new(),
            validator: None,
            violations: Vec::new(),
        }
    }

    /// Returns the renderer checking every frame it draws against the frame contract, starting
    /// from a renderer that holds nothing.
    ///
    /// The violations are collected in [`violations`](Self::violations).
    #[must_use]
    pub fn validating(mut self) -> Self {
        self.validator = Some(FrameValidator::new());
        self
    }

    /// Queues `outcome` as the answer to a later frame.
    ///
    /// Pushed outcomes answer frames in the order they were pushed.
    pub fn push_outcome(&mut self, outcome: FrameOutcome) {
        self.script.push_back(outcome);
    }

    /// Returns every frame drawn so far, oldest first.
    pub fn draws(&self) -> &[Draw] {
        &self.draws
    }

    /// Returns every violation found in the frames drawn so far, if the renderer validates.
    pub fn violations(&self) -> &[FrameViolation] {
        &self.violations
    }

    fn answer(&mut self, frame: &Frame) -> FrameOutcome {
        if let Some(outcome) = self.script.pop_front() {
            return outcome;
        }

        let Some(target) = self.target else {
            return FrameOutcome::Skipped(SkipReason::Unconfigured);
        };

        let damage_px = frame.damage.draw.area().map_or_else(
            || u64::from(target.size[0]) * u64::from(target.size[1]),
            |area| area.max(0) as u64,
        );
        FrameOutcome::Presented(FrameStats {
            vector_passes: frame.vector_plan.passes.len() as u32,
            damage_px,
            ..FrameStats::default()
        })
    }
}

impl Default for NullRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for NullRenderer {
    fn capabilities(&self) -> RenderCapabilities {
        self.caps
    }

    fn configure(&mut self, target: RenderTarget) {
        if self.target.is_some_and(|held| held.size != target.size) {
            self.kept = false;
        }
        self.held = false;
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
        if let Some(validator) = &mut self.validator {
            validator.set_capabilities(Some(self.caps));
            if let Err(found) = validator.check(frame) {
                self.violations.extend(found);
            }
        }

        let outcome = self.answer(frame);
        let lost = matches!(
            outcome,
            FrameOutcome::Recovered | FrameOutcome::Skipped(SkipReason::DeviceUnavailable)
        );
        if lost {
            self.kept = false;
            if let Some(validator) = &mut self.validator {
                validator.device_lost();
            }
        } else if outcome.retires_damage() {
            self.kept = true;
        }
        self.held = outcome == FrameOutcome::Skipped(SkipReason::Held);

        self.draws.push(Draw {
            frame: frame.id,
            output: frame.output,
            damage: frame.damage,
            shifts: frame.shifts.len(),
            outcome,
        });
        outcome
    }

    fn present_held(&mut self) -> FrameOutcome {
        if !self.held {
            return FrameOutcome::Skipped(SkipReason::Undamaged);
        }
        self.held = false;
        FrameOutcome::Presented(FrameStats::default())
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
