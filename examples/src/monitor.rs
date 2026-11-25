//! The refresh interval of the monitor a window is shown on.

use winit::window::Window;

/// Returns the refresh interval of the monitor `window` is shown on, in nanoseconds.
///
/// A window that has not entered a monitor yet takes the first monitor winit lists. Returns
/// `None` if no monitor reports a refresh rate.
pub fn refresh_ns(window: &Window) -> Option<u64> {
    let monitor = window
        .current_monitor()
        .or_else(|| window.available_monitors().next())?;
    let millihertz = u64::from(monitor.refresh_rate_millihertz()?);
    (millihertz > 0).then(|| 1_000_000_000_000 / millihertz)
}
