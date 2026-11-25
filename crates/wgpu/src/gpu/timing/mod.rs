//! Timestamp queries around a frame's passes, read back without stalling.

mod timer;

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) use self::timer::{GpuTimer, PassQueries};

/// The environment variable that turns GPU timing on.
pub const ENV: &str = "ICEBLINK_GPU_TIMING";

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Turns GPU timing on for devices opened after the call.
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

/// Reports whether GPU timing was turned on, by [`enable`] or by the environment.
pub fn requested() -> bool {
    static FROM_ENV: OnceLock<bool> = OnceLock::new();
    ENABLED.load(Ordering::Relaxed)
        || *FROM_ENV.get_or_init(|| {
            std::env::var_os(ENV).is_some_and(|value| !value.is_empty() && value != "0")
        })
}

/// A part of a frame's GPU work that is timed on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimedWork {
    /// Passes into the kept target.
    Draws,
    /// Passes into group targets.
    Groups,
    /// Path rasterisation.
    Vectors,
    /// The copy into a presented image.
    Blit,
}

/// The measured GPU time of one frame, by part.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GpuTiming {
    /// The frame's number, counting every frame from the renderer's first timed one.
    pub frame: u64,
    /// Milliseconds of [`TimedWork::Draws`].
    pub draws_ms: f64,
    /// Milliseconds of [`TimedWork::Groups`].
    pub groups_ms: f64,
    /// Milliseconds of [`TimedWork::Vectors`].
    pub vectors_ms: f64,
    /// Milliseconds of [`TimedWork::Blit`].
    pub blit_ms: f64,
    /// Passes that ran without a timestamp because the frame ran out of queries.
    pub untimed: u32,
}

impl GpuTiming {
    /// Returns the frame's total in milliseconds.
    pub fn total_ms(&self) -> f64 {
        self.draws_ms + self.groups_ms + self.vectors_ms + self.blit_ms
    }

    fn add(&mut self, work: TimedWork, ticks: u64, period_ns: f64) {
        let ms = ticks as f64 * period_ns * 1e-6;
        match work {
            TimedWork::Draws => self.draws_ms += ms,
            TimedWork::Groups => self.groups_ms += ms,
            TimedWork::Vectors => self.vectors_ms += ms,
            TimedWork::Blit => self.blit_ms += ms,
        }
    }
}
