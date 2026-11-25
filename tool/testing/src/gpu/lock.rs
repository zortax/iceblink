//! The lock that keeps device tests on one device at a time.

use std::sync::{Mutex, MutexGuard, OnceLock};

/// Takes the lock every device test of a test binary holds while it uses the device.
///
/// A test that panicked while holding the lock does not keep the next test off the device.
pub fn device_lock() -> MutexGuard<'static, ()> {
    // Real drivers do not survive several devices created and destroyed on several threads at
    // once, which is what parallel tests would otherwise do.
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let lock = LOCK.get_or_init(|| Mutex::new(()));
    lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
