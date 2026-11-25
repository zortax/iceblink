//! What drawing a frame did.

/// What drawing a frame did.
///
/// # Retiring damage
///
/// A frame retires its damage once its work is submitted, even when presenting fails afterwards.
/// Only the three outcomes that record nothing keep their damage: `Skipped(Unconfigured)`,
/// `Skipped(Unacquired)` and `Skipped(DeviceUnavailable)`. The compositor then draws that damage
/// again in the next frame of the output. `Recovered` retires the damage, and the next frame
/// redraws the whole output and resets the resident chunks.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameOutcome {
    /// The frame was presented.
    Presented(FrameStats),
    /// The frame was not presented.
    Skipped(SkipReason),
    /// The device was lost and rebuilt, and everything resident on it is gone.
    Recovered,
}

impl FrameOutcome {
    /// Reports whether the frame retires its damage, as described under "Retiring damage".
    pub const fn retires_damage(&self) -> bool {
        !matches!(
            self,
            Self::Skipped(
                SkipReason::Unconfigured | SkipReason::Unacquired | SkipReason::DeviceUnavailable
            )
        )
    }

    /// Reports whether the outcome asks for another frame immediately.
    ///
    /// The answer depends on the outcome:
    ///
    /// - `Presented` wants no other frame.
    /// - `Recovered` wants one.
    /// - `Skipped` with `Occluded`, `Undamaged`, `Held` or `DeviceUnavailable` wants none. These
    ///   wait for a platform event or the platform's readiness, or cannot improve.
    /// - Every other `Skipped` wants one.
    pub const fn wants_another_frame(&self) -> bool {
        match self {
            Self::Presented(_) => false,
            Self::Recovered => true,
            Self::Skipped(reason) => !matches!(
                reason,
                SkipReason::Occluded
                    | SkipReason::Undamaged
                    | SkipReason::Held
                    | SkipReason::DeviceUnavailable
            ),
        }
    }

    /// Returns the frame's statistics, if it was presented.
    pub const fn stats(&self) -> Option<FrameStats> {
        match self {
            Self::Presented(stats) => Some(*stats),
            _ => None,
        }
    }
}

/// Why a frame was not presented.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// No target is configured, or the presenter is not configured for it, so nothing was
    /// recorded.
    Unconfigured,
    /// Acquiring an image timed out after the frame was composed.
    Timeout,
    /// The display refused the composed frame because the image is still in flight.
    Declined,
    /// No image was free to compose into, or the kept target lost the contents the frame draws
    /// over, so nothing was recorded.
    Unacquired,
    /// The output is not visible.
    Occluded,
    /// The surface no longer matches and must be configured again.
    Outdated,
    /// The graphics API rejected the work.
    Validation,
    /// The device is lost and cannot be rebuilt, so nothing was recorded.
    DeviceUnavailable,
    /// The frame had no damage.
    Undamaged,
    /// The frame was composed and submitted, and its presentation waits for the platform.
    ///
    /// `Renderer::present_held` presents the frame once the platform is ready.
    Held,
}

/// What drawing one frame cost.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// Draw calls issued.
    pub draw_calls: u32,
    /// Batches skipped in a draw rectangle their ink misses.
    pub batches_culled: u32,
    /// Passes in the frame's vector plan.
    pub vector_passes: u32,
    /// Bytes copied to the device.
    pub bytes_uploaded: u64,
    /// Bytes of lowered chunks copied to the device, part of `bytes_uploaded`.
    pub chunk_bytes_uploaded: u64,
    /// Lowered chunks resident on the device after the frame.
    pub chunks_resident: u32,
    /// Pixels redrawn.
    pub damage_px: u64,
    /// Pixels copied from the kept target into the presented image.
    pub present_copy_px: u64,
    /// Pixels the presenter reported to the platform as changed.
    pub present_damage_px: u64,
    /// GPU time, when the device measures it.
    ///
    /// The time may describe an earlier frame.
    pub gpu_time_ns: Option<u64>,
    /// Passes of the vector plan drawn as nothing because the renderer has no path rasteriser.
    pub vector_passes_without_raster: u32,
}
