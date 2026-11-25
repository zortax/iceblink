//! Making the swap chain for a configuration.

use ash::vk;

use super::Chain;
use super::format::vk_format;
use crate::present::FRAME_LATENCY;
use crate::present::native::ChainConfig;

/// What the surface offers for one configuration.
struct Choice {
    format: vk::Format,
    view_formats: Vec<vk::Format>,
    color_space: vk::ColorSpaceKHR,
    mode: vk::PresentModeKHR,
    alpha: vk::CompositeAlphaFlagsKHR,
    extent: vk::Extent2D,
    count: u32,
    transform: vk::SurfaceTransformFlagsKHR,
    readable: bool,
}

impl Chain {
    /// Makes the swap chain again for `config`, and returns how many images it has.
    ///
    /// Fails with the reason when the surface does not offer `config` or a call fails.
    pub(crate) fn configure(&mut self, config: &ChainConfig) -> Result<usize, String> {
        let choice = self.choose(config)?;

        // Nothing in flight may still use an image this destroys, as wgpu's own configure waits.
        self.idle();
        self.release_images();

        self.replace_swapchain(&choice)?;
        self.extent = [choice.extent.width, choice.extent.height];
        self.readable = choice.readable;

        self.wrap_images(config)?;
        self.next = 0;
        Ok(self.images.len())
    }

    /// Makes a swap chain for `choice` in place of the current one.
    ///
    /// The device must be idle and the images released. The current swap chain is destroyed
    /// whether or not the new one is made.
    ///
    /// Fails with the reason when the swap chain cannot be made.
    fn replace_swapchain(&mut self, choice: &Choice) -> Result<(), String> {
        let usage = if choice.readable {
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC
        } else {
            vk::ImageUsageFlags::COLOR_ATTACHMENT
        };
        let mut listed = vec![choice.format];
        listed.extend(
            choice
                .view_formats
                .iter()
                .copied()
                .filter(|view| *view != choice.format),
        );
        let mut format_list = vk::ImageFormatListCreateInfo::default().view_formats(&listed);
        let old = self.swapchain;
        let mut info = vk::SwapchainCreateInfoKHR::default()
            .surface(self.surface)
            .min_image_count(choice.count)
            .image_format(choice.format)
            .image_color_space(choice.color_space)
            .image_extent(choice.extent)
            .image_array_layers(1)
            .image_usage(usage)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(choice.transform)
            .composite_alpha(choice.alpha)
            .present_mode(choice.mode)
            // An image that keeps its contents keeps every texel, covered by another window or
            // not.
            .clipped(false)
            .old_swapchain(old);
        if listed.len() > 1 {
            info = info
                .flags(vk::SwapchainCreateFlagsKHR::MUTABLE_FORMAT)
                .push_next(&mut format_list);
        }
        let held = self.timing.as_ref().map(|timing| timing.hold());
        // SAFETY: the surface is live and supported on this queue family, every field was checked
        // against what the surface offers, and `old` is this chain's own swap chain or null.
        let created = unsafe { self.swapchain_fns.create_swapchain(&info, None) };
        if old != vk::SwapchainKHR::null() {
            // SAFETY: the device is idle and the old swap chain's images are released. A
            // retired swap chain may be destroyed once nothing uses it, and the present waiter
            // is held off it.
            unsafe { self.swapchain_fns.destroy_swapchain(old, None) };
            self.swapchain = vk::SwapchainKHR::null();
        }
        let created = created.map_err(|error| format!("creating the swap chain: {error}"));
        self.swapchain = created.as_ref().copied().unwrap_or_default();
        // Present ids start again from 1 on every swap chain, and the presents before it number
        // the new chain's presents on.
        self.presents_before += self.present_id;
        self.present_id = 0;
        if let Some(timing) = &self.timing {
            timing.replaced(self.swapchain, self.presents_before);
        }
        self.refresh_ns = self.read_refresh();
        drop(held);
        created?;
        Ok(())
    }

    /// Wraps the images of a new swap chain made for `config`, and grows the acquire ring to one
    /// entry more than the images.
    ///
    /// Fails with the reason when a call fails.
    fn wrap_images(&mut self, config: &ChainConfig) -> Result<(), String> {
        // SAFETY: the swap chain is the live one `replace_swapchain` made.
        let raw_images = unsafe { self.swapchain_fns.get_swapchain_images(self.swapchain) }
            .map_err(|error| format!("the swap chain images: {error}"))?;
        for raw in raw_images {
            let image = self.wrap(raw, config)?;
            self.images.push(image);
        }
        while self.acquires.len() < self.images.len() + 1 {
            let acquire = self.ring_entry()?;
            self.acquires.push(acquire);
        }
        Ok(())
    }

    /// Chooses how the swap chain is made for `config`, among what the surface offers.
    ///
    /// Fails with the reason when the surface does not offer `config` or a call fails.
    fn choose(&self, config: &ChainConfig) -> Result<Choice, String> {
        let format = vk_format(config.format)
            .ok_or_else(|| format!("{:?} has no Vulkan format here", config.format))?;
        let view_formats: Vec<vk::Format> = config
            .view_formats
            .iter()
            .map(|format| vk_format(*format))
            .collect::<Option<_>>()
            .ok_or("a view format has no Vulkan format here")?;
        if !view_formats.is_empty() && !self.mutable_format {
            return Err("the device cannot view a swap chain in another format".to_owned());
        }

        // SAFETY: the physical device and the surface are live.
        let capabilities = unsafe {
            self.surface_fns
                .get_physical_device_surface_capabilities(self.physical, self.surface)
        }
        .map_err(|error| format!("the surface capabilities: {error}"))?;
        // SAFETY: as above.
        let surface_formats = unsafe {
            self.surface_fns
                .get_physical_device_surface_formats(self.physical, self.surface)
        }
        .map_err(|error| format!("the surface formats: {error}"))?;
        let color_space = surface_formats
            .iter()
            .filter(|offered| offered.format == format)
            .map(|offered| offered.color_space)
            .min_by_key(|space| *space != vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .ok_or_else(|| format!("the surface does not offer {format:?}"))?;
        // SAFETY: as above.
        let modes = unsafe {
            self.surface_fns
                .get_physical_device_surface_present_modes(self.physical, self.surface)
        }
        .map_err(|error| format!("the presentation modes: {error}"))?;
        let mode = wgpu::hal::vulkan::conv::map_present_mode(config.present_mode);
        if !modes.contains(&mode) {
            return Err(format!("the surface does not offer {mode:?}"));
        }
        let alpha = wgpu::hal::vulkan::conv::map_composite_alpha_mode(config.alpha_mode);
        if !capabilities.supported_composite_alpha.contains(alpha) {
            return Err(format!("the surface does not offer {alpha:?}"));
        }

        let extent = vk::Extent2D {
            width: config.size[0].max(1).clamp(
                capabilities.min_image_extent.width,
                capabilities.max_image_extent.width.max(1),
            ),
            height: config.size[1].max(1).clamp(
                capabilities.min_image_extent.height,
                capabilities.max_image_extent.height.max(1),
            ),
        };
        let wanted = FRAME_LATENCY + 1;
        let count = match capabilities.max_image_count {
            0 => wanted.max(capabilities.min_image_count),
            most => wanted.max(capabilities.min_image_count).min(most),
        };
        Ok(Choice {
            format,
            view_formats,
            color_space,
            mode,
            alpha,
            extent,
            count,
            transform: capabilities.current_transform,
            readable: capabilities
                .supported_usage_flags
                .contains(vk::ImageUsageFlags::TRANSFER_SRC),
        })
    }

    /// Returns the display's refresh interval, from `VK_GOOGLE_display_timing` where the device
    /// has it.
    fn read_refresh(&self) -> Option<u64> {
        let fns = self.display_timing.as_ref()?;
        if self.swapchain == vk::SwapchainKHR::null() {
            return None;
        }
        // SAFETY: the swap chain is live, and the caller holds it against the present waiter.
        let cycle = unsafe { fns.get_refresh_cycle_duration(self.swapchain) }.ok()?;
        (cycle.refresh_duration > 0).then_some(cycle.refresh_duration)
    }
}
