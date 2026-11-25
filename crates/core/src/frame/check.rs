//! A check every frame must pass while verification is on.

use iceblink::Frame;

use crate::compositor::CompositorImpl;

/// A check of a built frame.
///
/// The check returns a description of what the frame breaks, if it breaks anything.
pub type FrameCheck = Box<dyn FnMut(&Frame) -> Result<(), String> + Send>;

impl CompositorImpl {
    /// Sets the check every frame built while verification is on must pass, or removes it.
    ///
    /// A frame that fails the check is counted as `frame_check_failures` in `extra_stats`, and
    /// fails a debug assertion.
    pub fn set_frame_check(&mut self, check: Option<FrameCheck>) {
        self.frame_check = check;
    }

    /// Runs the frame check on a frame just built.
    pub(crate) fn check(&mut self, frame: &Frame) {
        let Some(check) = &mut self.frame_check else {
            return;
        };
        let failure = check(frame).err();
        if failure.is_some() {
            self.check_failures += 1;
        }

        debug_assert!(
            failure.is_none(),
            "frame {} of output {} breaks the frame contract:\n{}",
            frame.id.0,
            frame.output.0,
            failure.unwrap_or_default()
        );
    }
}
