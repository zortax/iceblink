//! The draw seam: executing finished frames.

use core::any::Any;
use core::time::Duration;

use crate::{
    BufferAge, Frame, FrameOutcome, MemoryReport, Presenter, RenderCapabilities, RenderTarget,
    SkipReason, SubpixelOrder, TargetPoolReport, VectorStatus,
};

/// Draws finished frames and presents them through its presenter.
///
/// A renderer is not bound to `Send`.
///
/// The defaults of the provided methods are the true answers for a renderer without the feature
/// the method is about.
pub trait Renderer {
    /// Returns what the device supports.
    ///
    /// The capabilities can change between frames, and every frame request includes the current
    /// ones.
    fn capabilities(&self) -> RenderCapabilities;

    /// Points the renderer at a new or resized target.
    fn configure(&mut self, target: RenderTarget);

    /// Returns the configured target, or `None` before the first `configure`.
    fn target(&self) -> Option<RenderTarget>;

    /// Returns what the kept target holds for the next frame.
    ///
    /// The age depends on the kept target:
    ///
    /// - If the kept target holds the last frame the renderer composed, including across a
    ///   `configure` that kept the picture, it is `Kept`.
    /// - If the renderer is in buffer-age mode, it is `Unknown`, and the age comes from the image
    ///   its presenter acquired.
    /// - Otherwise, such as after a device rebuild, it is `Unknown`.
    fn target_age(&self) -> BufferAge;

    /// Draws and presents `frame`.
    ///
    /// In buffer-age mode, the frame is composed into the image its presenter reports as
    /// `acquired`.
    ///
    /// The renderer applies the frame's resource work, chunk notes and table deltas even when it
    /// then skips the frame. A renderer that cannot apply them, such as one whose device was
    /// lost, returns `Recovered` or `Skipped(DeviceUnavailable)`. An outcome that retires damage
    /// applied the shifts, and an outcome that keeps damage applied none.
    ///
    /// In kept-target mode, the outcome also depends on the kept target and the last composed
    /// frame:
    ///
    /// - If `target_age` answered `Kept` for the frame, the kept target lost its contents before
    ///   `draw`, and the frame's draw damage is not the whole output, nothing is drawn and the
    ///   outcome is `Skipped(Unacquired)`, which keeps the damage.
    /// - If the frame has no draw damage and no shifts, the last composed frame is presented when
    ///   it was never presented, and otherwise the outcome is `Skipped(Undamaged)`.
    /// - If acquiring an image fails after the frame was composed, the outcome is
    ///   `Skipped(Timeout)`, so the composed frame's damage retires.
    fn draw(&mut self, frame: &Frame) -> FrameOutcome;

    /// Presents the frame that `draw` held back with `SkipReason::Held`, without drawing.
    ///
    /// While the platform still takes no image, the frame stays held and the outcome is
    /// `Skipped(Held)`. In kept-target mode, it also presents the last composed frame when that
    /// frame was never presented. A frame that a later `configure` made obsolete is not presented.
    /// The default holds no frame and returns `Skipped(Undamaged)`.
    fn present_held(&mut self) -> FrameOutcome {
        FrameOutcome::Skipped(SkipReason::Undamaged)
    }

    /// Returns how long the last frame waited for an image to present into.
    ///
    /// The default is zero.
    fn acquire_block(&self) -> Duration {
        Duration::ZERO
    }

    /// Sets the display's stripe order for subpixel sprites.
    ///
    /// The default ignores it.
    fn set_subpixel_order(&mut self, order: SubpixelOrder) {
        let _ = order;
    }

    /// Returns the presenter the renderer draws through, if it has one.
    ///
    /// The default is `None`.
    fn presenter(&mut self) -> Option<&mut dyn Presenter> {
        None
    }

    /// Returns the device memory the renderer holds, its path rasteriser's included.
    fn memory(&self) -> MemoryReport;

    /// Returns the pooled group targets the renderer holds.
    ///
    /// The default is an empty pool.
    fn target_pool(&self) -> TargetPoolReport {
        TargetPoolReport::default()
    }

    /// Frees pooled group targets that are not lent out, and returns the bytes freed.
    ///
    /// The default frees nothing.
    fn release_cached_targets(&mut self) -> u64 {
        0
    }

    /// Frees resources the renderer can recreate, after an idle period, and returns the bytes
    /// freed.
    ///
    /// The presentation state and an initialised path rasteriser are kept. The default releases
    /// the cached targets.
    fn release_idle_resources(&mut self) -> u64 {
        self.release_cached_targets()
    }

    /// Returns the path rasteriser the renderer is configured with.
    ///
    /// The default is no rasteriser.
    fn vector_status(&self) -> VectorStatus {
        VectorStatus::default()
    }

    /// Calls `out` with each renderer-specific counter, by name.
    ///
    /// The default reports none.
    fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        let _ = out;
    }

    /// Returns the renderer as its concrete type, for backend-specific companions.
    ///
    /// A wrapper renderer forwards the call to what it wraps. The default is `None`.
    fn as_any(&self) -> Option<&dyn Any> {
        None
    }

    /// Returns the renderer as its concrete type, mutably, for backend-specific companions.
    ///
    /// A wrapper renderer forwards the call to what it wraps. The default is `None`.
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        None
    }
}
