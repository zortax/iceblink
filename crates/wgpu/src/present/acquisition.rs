//! The answers a surface gives when asked for an image, and what the presenter does with each.

use iceblink::SkipReason;

/// What acquiring a surface image answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acquisition {
    /// An image was acquired.
    Success,
    /// An image was acquired, and the surface should be configured again.
    Suboptimal,
    /// No image became free in time.
    Timeout,
    /// The output is not visible.
    Occluded,
    /// The surface no longer matches its window.
    Outdated,
    /// The surface was lost and must be created again.
    Lost,
    /// The graphics API rejected the request.
    Validation,
}

impl Acquisition {
    /// Every answer, for parsing a fault to inject.
    pub const ALL: [Self; 7] = [
        Self::Success,
        Self::Suboptimal,
        Self::Timeout,
        Self::Occluded,
        Self::Outdated,
        Self::Lost,
        Self::Validation,
    ];

    /// Returns the answer's name, as a fault is spelled in the environment.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Suboptimal => "suboptimal",
            Self::Timeout => "timeout",
            Self::Occluded => "occluded",
            Self::Outdated => "outdated",
            Self::Lost => "lost",
            Self::Validation => "validation",
        }
    }

    /// Returns the answer `texture` holds.
    pub(crate) fn classify(texture: &wgpu::CurrentSurfaceTexture) -> Self {
        match texture {
            wgpu::CurrentSurfaceTexture::Success(_) => Self::Success,
            wgpu::CurrentSurfaceTexture::Suboptimal(_) => Self::Suboptimal,
            wgpu::CurrentSurfaceTexture::Timeout => Self::Timeout,
            wgpu::CurrentSurfaceTexture::Occluded => Self::Occluded,
            wgpu::CurrentSurfaceTexture::Outdated => Self::Outdated,
            wgpu::CurrentSurfaceTexture::Lost => Self::Lost,
            wgpu::CurrentSurfaceTexture::Validation => Self::Validation,
        }
    }

    /// Reports whether the answer gave an image to present.
    pub const fn presents(self) -> bool {
        matches!(self, Self::Success | Self::Suboptimal)
    }

    /// Reports whether the surface is configured again before the next acquisition.
    pub const fn reconfigures(self) -> bool {
        matches!(self, Self::Suboptimal | Self::Outdated | Self::Lost)
    }

    /// Returns the skip an answer without an image reports.
    ///
    /// A lost surface is created again by the presenter, so the frame reports it as outdated.
    pub const fn skip(self) -> SkipReason {
        match self {
            Self::Success | Self::Suboptimal | Self::Timeout => SkipReason::Timeout,
            Self::Occluded => SkipReason::Occluded,
            Self::Outdated | Self::Lost => SkipReason::Outdated,
            Self::Validation => SkipReason::Validation,
        }
    }
}
