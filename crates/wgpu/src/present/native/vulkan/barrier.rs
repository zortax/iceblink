//! The three layout transitions of a swap chain image, recorded once per image.

use ash::vk;

/// The colour aspect of one level and one layer.
const WHOLE: vk::ImageSubresourceRange = vk::ImageSubresourceRange {
    aspect_mask: vk::ImageAspectFlags::COLOR,
    base_mip_level: 0,
    level_count: 1,
    base_array_layer: 0,
    layer_count: 1,
};

/// A transition of a swap chain image between presenting and drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Barrier {
    /// The image's first frame: from `UNDEFINED` to the colour attachment layout.
    First,
    /// Every later frame: from the present layout to the colour attachment layout.
    Again,
    /// After a frame: from the colour attachment layout to the present layout.
    Release,
}

impl Barrier {
    /// Returns the image barrier over `image`.
    // The contents of an image survive a present and its reacquisition only while the barrier
    // that takes it back starts from `PRESENT_SRC_KHR`.
    pub(super) fn barrier(self, image: vk::Image) -> vk::ImageMemoryBarrier<'static> {
        let barrier = vk::ImageMemoryBarrier::default()
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(WHOLE);
        let attachment =
            vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE;
        match self {
            Self::First => barrier
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(attachment),
            Self::Again => barrier
                .old_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(attachment),
            Self::Release => barrier
                .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                .src_access_mask(attachment | vk::AccessFlags::TRANSFER_READ)
                .dst_access_mask(vk::AccessFlags::empty()),
        }
    }

    /// Returns the source and destination stages.
    // The acquire semaphore is waited at the colour attachment stage, so the acquire barriers
    // start there and the semaphore, the barrier and the copy after it form one dependency chain.
    // The release waits for the copy and for a readback that checks it.
    pub(super) fn stages(self) -> (vk::PipelineStageFlags, vk::PipelineStageFlags) {
        match self {
            Self::First | Self::Again => (
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            ),
            Self::Release => (
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            ),
        }
    }
}

/// Records `barrier` over `image` into `command`, for simultaneous use.
///
/// Fails with the driver's error.
pub(super) fn record(
    device: &ash::Device,
    command: vk::CommandBuffer,
    image: vk::Image,
    barrier: Barrier,
) -> Result<(), String> {
    let begin =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::SIMULTANEOUS_USE);
    let (source, destination) = barrier.stages();
    let barriers = [barrier.barrier(image)];
    // SAFETY: the command buffer was allocated from this device's pool and is in the initial
    // state, and the image belongs to this device.
    unsafe {
        device
            .begin_command_buffer(command, &begin)
            .map_err(|error| format!("recording a barrier: {error}"))?;
        device.cmd_pipeline_barrier(
            command,
            source,
            destination,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &barriers,
        );
        device
            .end_command_buffer(command)
            .map_err(|error| format!("recording a barrier: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use ash::vk;

    use super::Barrier;

    #[test]
    fn only_the_first_frame_of_an_image_discards_its_contents() {
        let image = vk::Image::null();
        assert_eq!(
            Barrier::First.barrier(image).old_layout,
            vk::ImageLayout::UNDEFINED
        );
        assert_eq!(
            Barrier::Again.barrier(image).old_layout,
            vk::ImageLayout::PRESENT_SRC_KHR
        );
        assert_eq!(
            Barrier::Release.barrier(image).new_layout,
            vk::ImageLayout::PRESENT_SRC_KHR
        );
    }

    #[test]
    fn the_acquire_barriers_start_at_the_stage_the_semaphore_is_waited_at() {
        for barrier in [Barrier::First, Barrier::Again] {
            assert!(
                barrier
                    .stages()
                    .0
                    .contains(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            );
        }
    }
}
