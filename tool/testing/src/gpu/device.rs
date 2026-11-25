//! A device for a test, on the driver the environment selects.

use std::sync::MutexGuard;

use crate::gpu::driver::{Driver, skip};
use crate::gpu::lock::device_lock;

/// A device and its queue on the requested driver, holding the device lock while it lives.
#[derive(Debug)]
pub struct TestDevice {
    /// The instance the adapter came from.
    pub instance: wgpu::Instance,
    /// The adapter the device was opened on.
    pub adapter: wgpu::Adapter,
    /// The device.
    pub device: wgpu::Device,
    /// The device's queue.
    pub queue: wgpu::Queue,
    /// The device lock, or `None` for a device opened beside another.
    _guard: Option<MutexGuard<'static, ()>>,
}

impl TestDevice {
    /// Opens a device on the requested driver with no optional features.
    ///
    /// Returns `None` after reporting the skip when the machine has no adapter of the requested
    /// driver or the adapter refuses a device. See [`skip`] for how a skip can fail the test
    /// instead.
    pub fn open() -> Option<Self> {
        Self::open_with(wgpu::Features::empty())
    }

    /// Opens a device on the requested driver with every feature of `wanted` the adapter has.
    ///
    /// The device gets the adapter's own limits. It returns `None` in the cases [`open`]
    /// describes.
    ///
    /// [`open`]: TestDevice::open
    pub fn open_with(wanted: wgpu::Features) -> Option<Self> {
        Self::opened(wanted, Some(device_lock()))
    }

    /// Opens another device with the features of this one, on an instance of its own.
    ///
    /// The device lock this device holds covers both. It returns `None` in the cases [`open`]
    /// describes.
    ///
    /// [`open`]: TestDevice::open
    pub fn beside(&self) -> Option<Self> {
        Self::opened(self.device.features(), None)
    }

    fn opened(wanted: wgpu::Features, guard: Option<MutexGuard<'static, ()>>) -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: wgpu::InstanceFlags::from_build_config().with_env(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = adapter(&instance)?;

        let descriptor = wgpu::DeviceDescriptor {
            label: Some("iceblink.test-device"),
            required_features: wanted & adapter.features(),
            required_limits: adapter.limits(),
            ..Default::default()
        };
        let (device, queue) = match pollster::block_on(adapter.request_device(&descriptor)) {
            Ok(opened) => opened,
            Err(error) => {
                return skip(&format!(
                    "{} refused a device: {error}",
                    describe(&adapter.get_info())
                ));
            }
        };

        Some(Self {
            instance,
            adapter,
            device,
            queue,
            _guard: guard,
        })
    }

    /// Returns what the adapter reports about itself.
    pub fn info(&self) -> wgpu::AdapterInfo {
        self.adapter.get_info()
    }

    /// Blocks until the device has finished all submitted work.
    pub fn wait(&self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }
}

/// Returns the first adapter of the requested driver, or reports the skip.
fn adapter(instance: &wgpu::Instance) -> Option<wgpu::Adapter> {
    let driver = Driver::requested();
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::PRIMARY));
    let considered: Vec<String> = adapters
        .iter()
        .map(|adapter| describe(&adapter.get_info()))
        .collect();
    let Some(adapter) = adapters
        .into_iter()
        .find(|adapter| driver.accepts(&adapter.get_info()))
    else {
        return skip(&format!(
            "no {driver:?} adapter among [{}]",
            considered.join(", ")
        ));
    };
    Some(adapter)
}

/// Describes an adapter on one line.
pub fn describe(info: &wgpu::AdapterInfo) -> String {
    format!(
        "{} ({:?}, {:?}, driver {} {})",
        info.name, info.device_type, info.backend, info.driver, info.driver_info
    )
}
