//! Checks of the frame contract that any frame from any compositor must meet.
//!
//! A frame stores its tables as deltas and refers to pages, shader slots and resident chunks that
//! earlier frames created. A [`FrameValidator`] follows those across the frames of one output, as
//! a renderer does. [`validate_frame`] checks a single frame on its own and reports only what that
//! frame proves wrong.
//!
//! The checks cover damage and shifts, page, shader and chunk-note operations, every lane source
//! and every frame slot it refers to, paint kinds, tiles and mark payloads, batch ranges, lane
//! coverage and bindings, group nesting and bounds, the vector plan, the mark plan, opaque covers
//! and whole glyph pixels. Paint order, coverage of damage and the vector plan's ordering and clip
//! rules depend on the scene the frame was built from, so no frame check decides them.

mod check;
mod known;
mod violation;

use iceblink::{Frame, RenderCapabilities};

use crate::validate::known::Known;

pub use self::violation::{FrameViolation, Rule, describe};

pub(crate) use self::check::rects::covered;

/// Checks `frame` on its own and returns every rule it breaks.
///
/// Tables, pages, shader slots and resident chunks the frame does not create are taken to be held
/// by the renderer, so an incremental frame passes as long as it is consistent with what it
/// shows. A slot past the end of a replaced table, a page the frame destroys and a revision it
/// retires are known to be gone.
pub fn validate_frame(frame: &Frame) -> Result<(), Vec<FrameViolation>> {
    FrameValidator::joining().check(frame)
}

/// Asserts that `frame` passes [`validate_frame`].
///
/// # Panics
///
/// Panics with every violation, one per line, if the frame breaks a rule.
#[track_caller]
pub fn assert_valid_frame(frame: &Frame) {
    if let Err(violations) = validate_frame(frame) {
        panic!(
            "frame {} breaks the frame contract:\n{}",
            frame.id.0,
            describe(&violations)
        );
    }
}

/// Checks the frames of one output in order, following what its renderer holds.
///
/// Every frame is applied as a renderer applies it, whatever its outcome: page and shader
/// operations, then chunk notes, then table deltas.
#[derive(Clone, Debug)]
pub struct FrameValidator {
    known: Known,
    caps: Option<RenderCapabilities>,
}

impl FrameValidator {
    /// Returns a validator for a renderer that has drawn nothing for the output.
    pub fn new() -> Self {
        Self {
            known: Known::empty(),
            caps: None,
        }
    }

    /// Returns a validator for a renderer whose earlier frames it has not seen.
    pub fn joining() -> Self {
        Self {
            known: Known::unknown(),
            caps: None,
        }
    }

    /// Sets the renderer capabilities every later frame is also checked against.
    ///
    /// With `None`, later frames are not checked against capabilities.
    pub fn set_capabilities(&mut self, caps: Option<RenderCapabilities>) {
        self.caps = caps;
    }

    /// Applies `frame` and returns every rule it breaks.
    pub fn check(&mut self, frame: &Frame) -> Result<(), Vec<FrameViolation>> {
        let violations = check::run(frame, &mut self.known, self.caps.as_ref());
        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }

    /// Forgets everything the renderer held, as after `FrameOutcome::Recovered`.
    pub fn device_lost(&mut self) {
        self.known = Known::empty();
    }
}

impl Default for FrameValidator {
    fn default() -> Self {
        Self::new()
    }
}
