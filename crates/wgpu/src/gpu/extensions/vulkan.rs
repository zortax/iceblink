//! Device extensions on the Vulkan backend.

use std::ffi::CStr;

use crate::gpu::adapter;
use crate::gpu::extensions::{Opened, PRESENT_TIMING};

/// Reports whether `adapter` has every one of `extensions`.
pub(crate) fn granted_by(adapter: &wgpu::Adapter, extensions: &[&'static CStr]) -> bool {
    if extensions.is_empty() {
        return true;
    }
    // SAFETY: `as_hal` requires that the adapter is not destroyed while the guard lives. The
    // guard is read and dropped here, and nothing destroys the adapter.
    let Some(hal) = (unsafe { adapter.as_hal::<wgpu::hal::api::Vulkan>() }) else {
        return false;
    };
    let physical = hal.physical_device_capabilities();
    extensions
        .iter()
        .all(|name| physical.supports_extension(name))
}

/// Reports whether the adapter has the present timing extensions and their features.
pub(crate) fn present_timing_granted(adapter: &wgpu::Adapter) -> bool {
    if !granted_by(adapter, &PRESENT_TIMING) {
        return false;
    }
    // SAFETY: as in `granted_by`.
    let Some(hal) = (unsafe { adapter.as_hal::<wgpu::hal::api::Vulkan>() }) else {
        return false;
    };
    let mut id = ash::vk::PhysicalDevicePresentIdFeaturesKHR::default();
    let mut wait = ash::vk::PhysicalDevicePresentWaitFeaturesKHR::default();
    let mut features = ash::vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut id)
        .push_next(&mut wait);
    // SAFETY: the physical device belongs to the instance, and the chain holds only feature
    // structures of extensions the device has.
    unsafe {
        hal.shared_instance()
            .raw_instance()
            .get_physical_device_features2(hal.raw_physical_device(), &mut features);
    }
    id.present_id == ash::vk::TRUE && wait.present_wait == ash::vk::TRUE
}

/// Opens a device on `adapter` with `extensions`.
///
/// The present timing extensions are opened with their features when `extensions` holds both
/// and the adapter has both features, and are left out otherwise.
///
/// Returns `None` when no extension is left to open, the adapter lacks one of them, or it
/// refuses the device.
pub(crate) fn open(
    adapter: &wgpu::Adapter,
    extensions: &[&'static CStr],
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Option<Opened> {
    let timing = PRESENT_TIMING.iter().all(|name| extensions.contains(name))
        && present_timing_granted(adapter);
    let extensions: Vec<&'static CStr> = extensions
        .iter()
        .copied()
        .filter(|name| timing || !PRESENT_TIMING.contains(name))
        .collect();
    if extensions.is_empty() {
        return None;
    }
    let named = || adapter::describe(&adapter.get_info());
    // SAFETY: `as_hal` requires that the adapter is not destroyed while the guard lives. The
    // guard is read and dropped here, and nothing destroys the adapter.
    let hal = unsafe { adapter.as_hal::<wgpu::hal::api::Vulkan>() }?;
    let physical = hal.physical_device_capabilities();
    if let Some(missing) = extensions
        .iter()
        .find(|name| !physical.supports_extension(name))
    {
        tracing::debug!(
            adapter = %named(),
            extension = ?missing,
            "the physical device does not have a Vulkan device extension that was asked for"
        );
        return None;
    }

    let opened = match create(&hal, &extensions, timing, descriptor) {
        Ok(opened) => opened,
        Err(error) => {
            tracing::warn!(
                adapter = %named(),
                %error,
                "a device with the Vulkan device extensions asked for was refused"
            );
            return None;
        }
    };
    let enabled = extensions;

    // SAFETY: `opened` was created from this adapter's hal adapter with `descriptor`, so the
    // descriptor's features are exactly the device's.
    match unsafe { adapter.create_device_from_hal(opened, descriptor) } {
        Ok((device, queue)) => Some((device, queue, enabled)),
        Err(error) => {
            tracing::warn!(
                adapter = %named(),
                %error,
                "a device created through the hal could not be adopted"
            );
            None
        }
    }
}

/// Opens a hal device with `extensions`, and with the present timing features when `timing`
/// holds.
fn create(
    hal: &wgpu::hal::vulkan::Adapter,
    extensions: &[&'static CStr],
    timing: bool,
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Result<wgpu::hal::OpenDevice<wgpu::hal::api::Vulkan>, wgpu::hal::DeviceError> {
    let mut id = ash::vk::PhysicalDevicePresentIdFeaturesKHR::default().present_id(true);
    let mut wait = ash::vk::PhysicalDevicePresentWaitFeaturesKHR::default().present_wait(true);
    // The features outlive the device creation, which reads them through the chain.
    let (id, wait) = (&mut id, &mut wait);
    let callback: Box<wgpu::hal::vulkan::CreateDeviceCallback<'_>> = Box::new(move |args| {
        for &name in extensions {
            if !args.extensions.contains(&name) {
                args.extensions.push(name);
            }
        }
        if timing {
            let info = std::mem::take(args.create_info);
            *args.create_info = info.push_next(id).push_next(wait);
        }
    });
    // SAFETY: the callback only appends extension names the physical device supports, which
    // `open` checked, and chains the present timing features only when `timing` holds, which
    // `open` sets only when `present_timing_granted` found both extensions and both features.
    unsafe {
        hal.open_with_callback(
            descriptor.required_features,
            &descriptor.required_limits,
            &descriptor.memory_hints,
            Some(callback),
        )
    }
}
