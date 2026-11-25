//! The buffers of a swap chain, wrapped for wgpu with the lists that move them.

use windows::Win32::Graphics::Direct3D12::{
    D3D12_COMMAND_LIST_TYPE_DIRECT, D3D12_RESOURCE_BARRIER, D3D12_RESOURCE_BARRIER_0,
    D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_BARRIER_FLAG_NONE,
    D3D12_RESOURCE_BARRIER_TYPE_TRANSITION, D3D12_RESOURCE_STATE_PRESENT,
    D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATES, D3D12_RESOURCE_TRANSITION_BARRIER,
    ID3D12CommandAllocator, ID3D12GraphicsCommandList, ID3D12Resource,
};

use super::Chain;

/// One buffer of the swap chain, with the lists that move it between frames and presents.
pub(super) struct Buffer {
    pub(super) texture: wgpu::Texture,
    /// The transition from the present state to the render target state.
    pub(super) again: ID3D12GraphicsCommandList,
    /// The transition from the render target state to the present state.
    pub(super) release: ID3D12GraphicsCommandList,
    /// The allocator both lists were recorded from.
    _allocator: ID3D12CommandAllocator,
    /// Whether a frame was drawn into the buffer since the swap chain was made or resized.
    pub(super) used: bool,
}

impl Chain {
    /// Wraps one buffer for wgpu and records the lists that move it.
    ///
    /// Fails with the reason when a call fails.
    pub(super) fn wrap(
        &self,
        resource: ID3D12Resource,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Result<Buffer, String> {
        // SAFETY: the device is live and the type is a direct list's.
        let allocator: ID3D12CommandAllocator = unsafe {
            self.device
                .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
        }
        .map_err(|error| format!("a command allocator: {error}"))?;
        let again = self.record(
            &allocator,
            &resource,
            D3D12_RESOURCE_STATE_PRESENT,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        )?;
        let release = self.record(
            &allocator,
            &resource,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_PRESENT,
        )?;

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        // SAFETY: the resource is a buffer of a swap chain made on this device, of this format and
        // extent, with one level and one sample. The texture holds a COM reference, which keeps
        // the buffer alive until wgpu drops it.
        let hal_texture = unsafe {
            wgpu::hal::dx12::Device::texture_from_raw(
                resource,
                format,
                wgpu::TextureDimension::D2,
                size,
                1,
                1,
            )
        };
        let descriptor = wgpu::TextureDescriptor {
            label: Some("iceblink.native.buffer"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        // SAFETY: the hal texture was made from this device's own buffer immediately above and
        // matches `descriptor`. A buffer of a new or resized swap chain is in the common state,
        // which `UNINITIALIZED` states. wgpu makes the first transition out of it.
        let texture = unsafe {
            self.gpu
                .device()
                .create_texture_from_hal::<wgpu::hal::api::Dx12>(
                    hal_texture,
                    &descriptor,
                    wgpu::TextureUses::UNINITIALIZED,
                )
        };
        Ok(Buffer {
            texture,
            again,
            release,
            _allocator: allocator,
            used: false,
        })
    }

    /// Records one transition of `resource` from `before` to `after` into a closed list.
    ///
    /// Fails with the reason when a call fails.
    fn record(
        &self,
        allocator: &ID3D12CommandAllocator,
        resource: &ID3D12Resource,
        before: D3D12_RESOURCE_STATES,
        after: D3D12_RESOURCE_STATES,
    ) -> Result<ID3D12GraphicsCommandList, String> {
        // SAFETY: the allocator is the device's own and of the direct type.
        let list: ID3D12GraphicsCommandList = unsafe {
            self.device
                .CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, allocator, None)
        }
        .map_err(|error| format!("a command list: {error}"))?;
        let barrier = D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: core::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    // SAFETY: a borrowed copy of the reference, with no reference count taken.
                    // `ManuallyDrop` keeps it from being released, and the list keeps the barrier
                    // only while it is recorded, while `resource` is borrowed.
                    pResource: unsafe { core::mem::transmute_copy(resource) },
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: before,
                    StateAfter: after,
                }),
            },
        };
        // SAFETY: the list is open, and the barrier names a live resource.
        unsafe {
            list.ResourceBarrier(&[barrier]);
            list.Close()
        }
        .map_err(|error| format!("closing a command list: {error}"))?;
        Ok(list)
    }
}
