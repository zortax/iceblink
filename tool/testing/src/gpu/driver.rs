//! Which driver a device test runs on, and what a test does when it has none.

/// The environment variable the Vulkan loader reads to select drivers.
pub const DRIVERS_VARIABLE: &str = "VK_LOADER_DRIVERS_SELECT";

/// The environment variable that turns a skipped device test into a failure.
pub const REQUIRE_VARIABLE: &str = "ICEBLINK_REQUIRE_GPU";

/// The kind of driver a device test runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Driver {
    /// A hardware adapter, such as an NVIDIA card.
    Hardware,
    /// Mesa's software Vulkan driver, lavapipe.
    Lavapipe,
}

impl Driver {
    /// Returns the driver the environment selects.
    ///
    /// The driver is lavapipe when `VK_LOADER_DRIVERS_SELECT` contains `lvp` in any case, and
    /// hardware otherwise.
    pub fn requested() -> Self {
        let selects_lavapipe = std::env::var(DRIVERS_VARIABLE)
            .is_ok_and(|value| value.to_ascii_lowercase().contains("lvp"));
        if selects_lavapipe {
            Self::Lavapipe
        } else {
            Self::Hardware
        }
    }

    /// Reports whether `info` describes an adapter of this driver.
    ///
    /// Lavapipe is the adapter that rasterises on the processor, and every other adapter is
    /// hardware.
    pub fn accepts(self, info: &wgpu::AdapterInfo) -> bool {
        let software = info.device_type == wgpu::DeviceType::Cpu;
        match self {
            Self::Hardware => !software,
            Self::Lavapipe => software,
        }
    }
}

/// Reports a device test that cannot run, and returns `None`.
///
/// # Panics
///
/// Panics instead when `ICEBLINK_REQUIRE_GPU` is set to anything but an empty value or `0`.
#[track_caller]
pub fn skip<T>(reason: &str) -> Option<T> {
    let required =
        std::env::var(REQUIRE_VARIABLE).is_ok_and(|value| !value.is_empty() && value != "0");
    if required {
        panic!("no device for a required device test: {reason}");
    }
    eprintln!("skipped: {reason}");
    None
}
