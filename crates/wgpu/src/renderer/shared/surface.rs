//! Presenters on a window surface made on an open device.

use std::rc::Rc;

use crate::present::native::NativeWindow;
use crate::present::{SurfacePresenter, WgpuPresenter};
use crate::renderer::builder::WindowSurface;
use crate::renderer::shared::DeviceState;

/// Makes a presenter for `surface` on the device of `state`.
///
/// Fails with [`Rejected::Incompatible`] when the device cannot present to the window, and with
/// [`Rejected::Failed`] when making the presenter raised a validation error.
pub(super) fn present_on<W: Into<wgpu::SurfaceTarget<'static>> + Clone>(
    state: &Rc<DeviceState>,
    surface: &mut WindowSurface<W>,
) -> Result<Box<dyn WgpuPresenter>, Rejected> {
    let made = state
        .gpu
        .instance()
        .create_surface(surface.window.clone())
        .map_err(|_| Rejected::Incompatible)?;
    if made
        .get_capabilities(state.gpu.adapter())
        .formats
        .is_empty()
    {
        return Err(Rejected::Incompatible);
    }

    let window = NativeWindow::of(surface.window.clone());
    let make = || {
        let mut presenter = SurfacePresenter::unconfigured(
            &state.gpu,
            made,
            surface.size,
            surface.opaque,
            surface.pacing,
        );
        presenter.set_window(window);
        presenter.configure_now();
        presenter
    };
    // A browser reports a validation error only once this thread returns to its event loop.
    #[cfg(target_arch = "wasm32")]
    {
        Ok(Box::new(make()))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let scope = state
            .gpu
            .device()
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let presenter = make();
        match futures::executor::block_on(scope.pop()) {
            None => Ok(Box::new(presenter)),
            Some(error) => Err(Rejected::Failed(error.to_string())),
        }
    }
}

/// Why a device makes no presenter for a window.
pub(super) enum Rejected {
    /// The device cannot present to the window.
    Incompatible,
    /// The presenter raised a validation error.
    #[cfg_attr(target_arch = "wasm32", expect(dead_code))]
    Failed(String),
}
