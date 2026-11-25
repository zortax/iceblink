//! The trait the renderer accesses a presenter's images through, and the pixmap presenter.

mod debt;
mod pixmap;

use iceblink::Presenter;

use crate::target::Pixmap;

pub use self::debt::ImageDebts;
pub use self::pixmap::PixmapPresenter;

/// A presenter whose images are pixmaps the CPU renderer copies or draws into.
///
/// A presenter in kept-target mode receives copies of the renderer's kept target, and one in
/// buffer-age mode is drawn into directly.
pub trait CpuPresenter: Presenter {
    /// Returns the extent of the presenter's images, in texels.
    fn extent(&self) -> [u32; 2];

    /// Returns image `image`, if it exists.
    fn image(&self, image: u32) -> Option<&Pixmap>;

    /// Returns image `image` to write into, if it exists.
    fn image_mut(&mut self, image: u32) -> Option<&mut Pixmap>;

    /// Reports whether the images match the extent the presenter was last configured for.
    ///
    /// A renderer draws nothing into a presenter that does not.
    fn is_configured(&self) -> bool {
        true
    }
}
