//! What paces presentation, and the presentation mode it asks for.

/// What paces presentation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PresentPacing {
    /// The display's refresh, through first-in first-out presentation.
    #[default]
    Display,
    /// The platform's own frame callbacks, through mailbox or immediate presentation.
    Platform,
}

/// Chooses the presentation mode for `pacing` among what the surface offers.
///
/// Platform pacing takes mailbox, then immediate presentation, and falls back to first-in
/// first-out presentation where the surface offers neither.
pub(crate) fn present_mode(
    pacing: PresentPacing,
    capabilities: &wgpu::SurfaceCapabilities,
) -> wgpu::PresentMode {
    if pacing == PresentPacing::Display {
        return wgpu::PresentMode::Fifo;
    }
    [wgpu::PresentMode::Mailbox, wgpu::PresentMode::Immediate]
        .into_iter()
        .find(|mode| capabilities.present_modes.contains(mode))
        .unwrap_or(wgpu::PresentMode::Fifo)
}
