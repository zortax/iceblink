//! The presenters of the wgpu renderer and the trait through which the renderer draws into their
//! images.
//!
//! Every presenter implements [`iceblink::Presenter`]. [`WgpuPresenter`] adds what the
//! renderer needs on the device: the images' formats and textures, and whether they keep their
//! contents. A presenter in kept-target mode receives copies of the kept target, and one in
//! buffer-age mode is drawn into directly.

pub(crate) mod acquisition;
mod debt;
mod fault;
pub(crate) mod native;
mod offscreen;
mod supplied;
mod surface;

use std::sync::Arc;

use iceblink::GpuUnavailable;
use iceblink::Presenter;

use crate::gpu::device::Gpu;
use crate::gpu::formats::Formats;

pub use self::acquisition::Acquisition;
pub use self::debt::ImageDebts;
pub use self::fault::{FaultInjector, VARIABLE as SURFACE_FAULT_VARIABLE};
pub use self::native::{PRESENTER_VARIABLE, SwapChain, TIMING_VARIABLE, VERIFY_VARIABLE};
pub use self::offscreen::Offscreen;
pub use self::supplied::Supplied;
pub use self::surface::{FRAME_LATENCY, PresentPacing, SurfacePresenter};

/// A presenter whose images the wgpu renderer copies or draws into.
pub trait WgpuPresenter: Presenter {
    /// Returns the formats of the presenter's images.
    fn formats(&self) -> Formats;

    /// Returns the extent of the presenter's images, in texels.
    fn extent(&self) -> [u32; 2];

    /// Returns the texture of an acquired image.
    fn image_texture(&self, image: u32) -> Option<&wgpu::Texture>;

    /// Returns the view of an acquired image to write through, in [`Formats::present_attachment`].
    fn image_view(&self, image: u32) -> Option<&wgpu::TextureView>;

    /// Reports whether an image keeps its texels between presents, so a copy into it covers
    /// only its debt.
    fn keeps_contents(&self) -> bool;

    /// Reports whether the images match the extent the presenter was last configured for.
    ///
    /// A renderer draws nothing into a presenter that does not.
    fn is_configured(&self) -> bool {
        true
    }

    /// Checks the acquired `image` against `composed`, the kept target its debt was copied from,
    /// over `size` texels.
    ///
    /// Only a presenter asked to check its images does anything.
    fn check_image(&self, image: u32, composed: &wgpu::Texture, size: [u32; 2]) {
        let _ = (image, composed, size);
    }

    /// Takes the refreshes the frame about to be presented was planned on: one at `refresh_ns`
    /// and the others every `interval_ns` from it, on the frame clock.
    ///
    /// A presenter that times its presents may wait for them only around those refreshes.
    fn follow_refreshes(&mut self, refresh_ns: u64, interval_ns: u64) {
        let _ = (refresh_ns, interval_ns);
    }

    /// Reports whether as many presented images wait for the display as it holds at once, so an
    /// image presented now would queue behind them.
    fn display_full(&self) -> bool {
        false
    }

    /// Moves the presenter onto a rebuilt device.
    ///
    /// Fails when the presenter's images cannot be created on the new device by the presenter
    /// itself, as for a window surface, which its owner creates again.
    fn rebuild(&mut self, gpu: &Arc<Gpu>) -> Result<(), GpuUnavailable>;
}
