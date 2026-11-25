//! Device loss, as the device reports it or as a run of failed acquisitions implies it.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

/// The function called once when the device is lost.
struct Watcher(Box<dyn Fn() + Send + Sync>);

impl core::fmt::Debug for Watcher {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("Watcher")
    }
}

/// Whether a device was lost, and the run of failed acquisitions that counts as a loss.
#[derive(Debug, Default)]
pub struct DeviceLoss {
    lost: AtomicBool,
    consecutive_validation_failures: AtomicU32,
    watcher: OnceLock<Watcher>,
}

impl DeviceLoss {
    /// The length of a run of acquisitions failing validation that counts as a loss.
    pub const VALIDATION_FAILURE_LIMIT: u32 = 10;

    /// Returns a device that is not lost.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks the device lost and logs the report.
    ///
    /// The watcher is called on the first report.
    pub fn report(&self, reason: wgpu::DeviceLostReason, message: &str) {
        let already = self.lost.swap(true, Ordering::AcqRel);
        tracing::error!(?reason, message, "the graphics device was lost");
        if !already && let Some(watcher) = self.watcher.get() {
            (watcher.0)();
        }
    }

    /// Sets the function called once when the device is lost.
    ///
    /// Only the first watcher is kept.
    pub fn watch(&self, told: impl Fn() + Send + Sync + 'static) {
        let _ = self.watcher.set(Watcher(Box::new(told)));
    }

    /// Returns a device lost callback that calls [`report`](Self::report) on this loss.
    pub fn observer(
        self: &Arc<Self>,
    ) -> impl Fn(wgpu::DeviceLostReason, &str) + Send + Sync + 'static {
        let loss = Arc::clone(self);
        move |reason, message| loss.report(reason, message)
    }

    /// Reports whether the device was lost.
    pub fn is_lost(&self) -> bool {
        self.lost.load(Ordering::Acquire)
    }

    /// Counts an acquisition that failed validation, and reports whether the run reached the
    /// limit.
    pub fn note_validation_failure(&self) -> bool {
        let seen = self
            .consecutive_validation_failures
            .fetch_add(1, Ordering::AcqRel)
            + 1;
        seen >= Self::VALIDATION_FAILURE_LIMIT
    }

    /// Ends the run of failed acquisitions.
    pub fn note_acquisition_succeeded(&self) {
        self.consecutive_validation_failures
            .store(0, Ordering::Release);
    }

    /// Returns the length of the current run of failed acquisitions.
    pub fn consecutive_validation_failures(&self) -> u32 {
        self.consecutive_validation_failures.load(Ordering::Acquire)
    }

    /// Marks the device as usable again.
    pub fn clear(&self) {
        self.lost.store(false, Ordering::Release);
        self.consecutive_validation_failures
            .store(0, Ordering::Release);
    }
}
