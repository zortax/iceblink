//! What a native chain needs to know about a window beyond its wgpu surface.

/// The handles of a window that a native chain is made for.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct NativeWindow {
    /// The Win32 window handle, if the window has one.
    #[cfg_attr(
        not(windows),
        expect(
            dead_code,
            reason = "only a DXGI swap chain is made for a window handle"
        )
    )]
    pub(crate) hwnd: Option<isize>,
}

impl NativeWindow {
    /// Returns the handles `window` reports.
    pub(crate) fn of(window: impl Into<wgpu::SurfaceTarget<'static>>) -> Self {
        use wgpu::rwh::{HasWindowHandle, RawWindowHandle};

        let raw = match window.into() {
            wgpu::SurfaceTarget::DisplayAndWindow(handle) => {
                handle.window_handle().ok().map(|handle| handle.as_raw())
            }
            wgpu::SurfaceTarget::Window(handle) => {
                handle.window_handle().ok().map(|handle| handle.as_raw())
            }
            _ => None,
        };
        let hwnd = match raw {
            Some(RawWindowHandle::Win32(handle)) => Some(handle.hwnd.get()),
            _ => None,
        };
        Self { hwnd }
    }
}
