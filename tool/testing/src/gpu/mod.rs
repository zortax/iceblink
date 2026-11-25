//! Device tests: a device on the requested driver, the lock that serialises tests on it, and
//! texture readback.
//!
//! A test runs on hardware by default and on lavapipe when `VK_LOADER_DRIVERS_SELECT` contains
//! `lvp`. A test that finds no device skips with a `skipped:` line, or fails when
//! `ICEBLINK_REQUIRE_GPU` is set.

mod device;
mod driver;
mod lock;
mod readback;

pub use self::device::{TestDevice, describe};
pub use self::driver::{DRIVERS_VARIABLE, Driver, REQUIRE_VARIABLE, skip};
pub use self::lock::device_lock;
pub use self::readback::read_rgba;
