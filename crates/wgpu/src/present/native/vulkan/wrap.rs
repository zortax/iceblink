//! Wrapping swap chain images for wgpu, and the entries of the acquire ring.

use ash::vk;

use super::barrier::{Barrier, record};
use super::{Acquire, Chain, Image};
use crate::present::native::ChainConfig;

impl Chain {
    /// Wraps one swap chain image for wgpu and records the barriers that move it.
    ///
    /// Fails with the reason when the device is not a Vulkan device or a call fails.
    pub(super) fn wrap(&mut self, raw: vk::Image, config: &ChainConfig) -> Result<Image, String> {
        let size = wgpu::Extent3d {
            width: self.extent[0],
            height: self.extent[1],
            depth_or_array_layers: 1,
        };
        let (uses, usage) = if self.readable {
            (
                wgpu::TextureUses::COLOR_TARGET | wgpu::TextureUses::COPY_SRC,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            )
        } else {
            (
                wgpu::TextureUses::COLOR_TARGET,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            )
        };
        let hal_descriptor = wgpu::hal::TextureDescriptor {
            label: Some("iceblink.native.image"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: config.format,
            usage: uses,
            memory_flags: wgpu::hal::MemoryFlags::empty(),
            view_formats: config.view_formats.clone(),
        };
        // SAFETY: the device guard is read through and dropped here. The image belongs to the
        // swap chain and lives until the swap chain is destroyed, after the texture is dropped and
        // the device is idle. The drop callback does nothing, so wgpu never destroys it. The
        // descriptor states the image's own extent, format, usage and view formats, and the swap
        // chain is mutable-format exactly when there are view formats.
        let hal_texture = unsafe {
            let hal = self
                .gpu
                .device()
                .as_hal::<wgpu::hal::api::Vulkan>()
                .ok_or("the device is not a Vulkan device")?;
            hal.texture_from_raw(
                raw,
                &hal_descriptor,
                Some(Box::new(|| {})),
                wgpu::hal::vulkan::TextureMemory::External,
            )
        };
        let descriptor = wgpu::TextureDescriptor {
            label: Some("iceblink.native.image"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: config.format,
            usage,
            view_formats: &config.view_formats,
        };
        // SAFETY: the hal texture was made from this device's own image immediately above and
        // matches `descriptor`. An image just taken from a swap chain is in `UNDEFINED`, which is
        // what `UNINITIALIZED` states.
        let texture = unsafe {
            self.gpu
                .device()
                .create_texture_from_hal::<wgpu::hal::api::Vulkan>(
                    hal_texture,
                    &descriptor,
                    wgpu::TextureUses::UNINITIALIZED,
                )
        };

        let info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(3);
        // SAFETY: the pool belongs to this device and only this thread allocates from it.
        let commands = unsafe { self.device.allocate_command_buffers(&info) }
            .map_err(|error| format!("allocating the barriers: {error}"))?;
        let (first, again, release) = (commands[0], commands[1], commands[2]);
        record(&self.device, first, raw, Barrier::First)?;
        record(&self.device, again, raw, Barrier::Again)?;
        record(&self.device, release, raw, Barrier::Release)?;
        // SAFETY: the structure is Vulkan's own with its `sType` set.
        let rendered = unsafe {
            self.device
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
        }
        .map_err(|error| format!("creating a semaphore: {error}"))?;
        Ok(Image {
            texture,
            first,
            again,
            release,
            rendered,
            used: false,
        })
    }

    /// Makes one more entry of the acquire ring.
    ///
    /// Fails with the reason when a call fails.
    pub(super) fn ring_entry(&self) -> Result<Acquire, String> {
        // SAFETY: the structures are Vulkan's own with their `sType` set.
        let semaphore = unsafe {
            self.device
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
        }
        .map_err(|error| format!("creating a semaphore: {error}"))?;
        // SAFETY: as above.
        let fence = match unsafe {
            self.device
                .create_fence(&vk::FenceCreateInfo::default(), None)
        } {
            Ok(fence) => fence,
            Err(error) => {
                // SAFETY: the semaphore was created on this device immediately above and nothing
                // has used it.
                unsafe { self.device.destroy_semaphore(semaphore, None) };
                return Err(format!("creating a fence: {error}"));
            }
        };
        Ok(Acquire {
            semaphore,
            fence,
            pending: false,
        })
    }
}
