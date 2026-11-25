//! The rasteriser contract.

use iceblink::{MemoryReport, VectorBackend, VectorPlan};

use crate::vector::frame::VectorFrame;
use crate::vector::resource::{ResourcedPlan, VectorError, VectorTarget};

/// A vector rasteriser that draws a frame's vector passes into scratch targets for the renderer to
/// composite.
///
/// A rasteriser writes straight, unpremultiplied colour in an unencoded eight-bit format, with one
/// texel per device pixel, into the raster region assigned to each pass. Anything a pass does not
/// paint reads as fully transparent. The compositing draw premultiplies.
///
/// The frame's vector plan decides which paths are drawn, how they split into passes, the clip of
/// each pass and whether a pass composites one item at a time. A rasteriser decides only its own
/// resources, and it draws every planned path without culling it against damage.
///
/// A path whose clip chain is deeper than its pass's clip carries the extra links as residual
/// clips, and the rasteriser applies them inside the scratch.
///
/// The renderer calls the methods in this sequence each frame:
///
/// 1. [`plan`](Self::plan) once.
/// 2. For each round, [`clear_targets`](Self::clear_targets), then [`prepare`](Self::prepare),
///    then the renderer records the composites of that round and reads [`view`](Self::view).
///
/// A renderer that keeps rasters by pass key may composite a kept raster for a keyed pass
/// without rasterising it. It holds the `Arc`s of the pass's outlines while it keeps the raster.
pub trait VectorRaster: 'static {
    /// Reports which backend this is.
    ///
    /// Reading it never initialises a lazily created rasteriser.
    fn backend(&self) -> VectorBackend {
        VectorBackend::Other
    }

    /// Reports whether the rasteriser fills a gradient path with one colour, the mean colour of
    /// its ramp as [`flat`](crate::vector::paint::flat) computes it.
    ///
    /// The default is false, so gradients are drawn as the frame says.
    fn flat_gradients(&self) -> bool {
        false
    }

    /// Resources the frame's vector plan.
    ///
    /// The result is index-aligned with `plan`: one resourced pass per planned pass, in the same
    /// order, or an empty plan when nothing is drawn. [`ResourcedPlan::resourcing`] starts a plan
    /// that keeps the alignment. A pass the rasteriser cannot place gets [`VectorTarget::NONE`].
    fn plan(&mut self, plan: &VectorPlan) -> ResourcedPlan;

    /// Clears every scratch target that `round` of `plan` writes.
    fn clear_targets(&mut self, plan: &ResourcedPlan, round: usize);

    /// Rasterises every pass of the frame's round.
    ///
    /// Rounds are prepared in order, and a round may reuse the targets of the one before it. A
    /// frame with several damage rectangles prepares the same round again for each rectangle.
    ///
    /// # Errors
    ///
    /// Returns a [`VectorError`] when a pass could not be drawn, including a pass that did not fit
    /// its target.
    fn prepare(&mut self, frame: &mut VectorFrame<'_>) -> Result<(), VectorError>;

    /// Returns the texture view holding the passes placed in `target`.
    ///
    /// Returns `None` for a target this rasteriser did not create.
    fn view(&self, target: VectorTarget) -> Option<&wgpu::TextureView>;

    /// Returns the texture holding the passes placed in `target`, and the array layer they are
    /// in.
    ///
    /// A pass's raster can be copied out of it once the pass's round is prepared. The default is
    /// `None`, which keeps no raster.
    fn texture(&self, target: VectorTarget) -> Option<(&wgpu::Texture, u32)> {
        let _ = target;
        None
    }

    /// Reports the memory this rasteriser holds.
    fn memory(&self) -> MemoryReport;

    /// Releases scratch that a later frame can recreate, and returns the bytes released.
    ///
    /// Initialised pipelines and fixed state stay resident. The default releases nothing.
    fn release_idle_resources(&mut self) -> u64 {
        0
    }

    /// Reports rasteriser-specific counters by name.
    ///
    /// The default reports none.
    fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        let _ = out;
    }
}
