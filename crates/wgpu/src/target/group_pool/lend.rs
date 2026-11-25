//! Lending targets: reusing a held one, or allocating one within the budget.

use iceblink::geom::{Device, Rect, Size};

use crate::gpu::device::Gpu;
use crate::target::group_pool::cover::{allocated_for, cover_for};
use crate::target::group_pool::{Entry, GroupPool, GroupSlot, texture_bytes, trim};
use crate::target::scale::TargetScale;

impl GroupPool {
    /// Lends a target at `scale` in `format` that covers `region`, or one at half resolution when
    /// the budget leaves no room for it.
    ///
    /// Returns `None` when neither fits the budget.
    pub(crate) fn acquire_covering(
        &mut self,
        gpu: &Gpu,
        scale: TargetScale,
        format: wgpu::TextureFormat,
        region: Rect<i32, Device>,
    ) -> Option<GroupSlot> {
        let lent = self.lent();
        let cover = self.cover(region);
        let slot = self
            .reuse(scale, format, cover)
            .or_else(|| self.allocate(gpu, scale, format, cover))
            .or_else(|| {
                let degraded = self
                    .reuse(TargetScale::Half, format, cover)
                    .or_else(|| self.allocate(gpu, TargetScale::Half, format, cover));
                if degraded.is_some() && scale != TargetScale::Half {
                    self.degraded += 1;
                    tracing::debug!(
                        budget = self.budget,
                        resident = self.bytes(),
                        "isolating at half resolution because the target budget is full"
                    );
                }
                degraded
            })?;
        self.count(lent);
        Some(slot)
    }

    /// Lends a target at `scale` in `format` that covers `region`, or returns `None` when it does
    /// not fit the budget.
    pub(crate) fn acquire_exact_covering(
        &mut self,
        gpu: &Gpu,
        scale: TargetScale,
        format: wgpu::TextureFormat,
        region: Rect<i32, Device>,
    ) -> Option<GroupSlot> {
        let lent = self.lent();
        let cover = self.cover(region);
        let slot = self
            .reuse(scale, format, cover)
            .or_else(|| self.allocate(gpu, scale, format, cover))?;
        self.count(lent);
        Some(slot)
    }

    /// Counts a lease granted while `lent` targets were lent out.
    fn count(&mut self, lent: u32) {
        self.peak = self.peak.max(lent + 1);
        self.leases += 1;
    }

    /// Lends the smallest unlent target at `scale` in `format` that holds `cover`.
    fn reuse(
        &mut self,
        scale: TargetScale,
        format: wgpu::TextureFormat,
        cover: Rect<i32, Device>,
    ) -> Option<GroupSlot> {
        let needed = scale.extent(cover.size);
        let mut best: Option<(usize, u64)> = None;
        for (index, held) in self.entries.iter().enumerate() {
            let Some(entry) = held else {
                continue;
            };
            if entry.lent
                || entry.scale != scale
                || entry.format != format
                || entry.extent.width < needed.width
                || entry.extent.height < needed.height
            {
                continue;
            }
            let area = entry.extent.width as u64 * entry.extent.height as u64;
            if best.is_none_or(|(_, smallest)| area < smallest) {
                best = Some((index, area));
            }
        }
        let (index, _) = best?;
        let entry = self.entries[index].as_mut()?;
        entry.lent = true;
        entry.touched = true;
        Some(GroupSlot {
            index,
            scale,
            format,
            origin: cover.origin,
            covered: cover.size,
            allocated: entry.extent,
        })
    }

    /// Allocates and lends a target at `scale` in `format` for `cover`, and frees targets unused
    /// this frame to make room for it.
    ///
    /// Returns `None` when the lent targets leave no room for it in the budget.
    fn allocate(
        &mut self,
        gpu: &Gpu,
        scale: TargetScale,
        format: wgpu::TextureFormat,
        cover: Rect<i32, Device>,
    ) -> Option<GroupSlot> {
        let allocated = allocated_for(cover.size, scale);
        let cost = u64::from(format.block_copy_size(None).unwrap_or(8))
            * allocated.width.max(1) as u64
            * allocated.height.max(1) as u64;
        if self.bytes() + cost > self.budget {
            if self.lent_bytes() + cost > self.budget {
                return None;
            }
            self.evict(cost);
        }
        let extent = Size::new(allocated.width.max(1), allocated.height.max(1));
        let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("iceblink.group"),
            size: wgpu::Extent3d {
                width: extent.width as u32,
                height: extent.height as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: Self::USAGE,
            view_formats: &[],
        });
        self.allocations += 1;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let entry = Entry {
            texture,
            view,
            extent,
            scale,
            format,
            lent: true,
            touched: true,
            idle: 0,
        };
        let index = match self.entries.iter().position(Option::is_none) {
            Some(hole) => {
                self.entries[hole] = Some(entry);
                hole
            }
            None => {
                self.entries.push(Some(entry));
                self.entries.len() - 1
            }
        };
        Some(GroupSlot {
            index,
            scale,
            format,
            origin: cover.origin,
            covered: cover.size,
            allocated: extent,
        })
    }

    /// Frees targets unused this frame until `cost` more bytes fit the budget.
    fn evict(&mut self, cost: u64) {
        let idle = |entry: &Entry| !entry.lent && !entry.touched;
        let mut resident = self.bytes();
        for held in &mut self.entries {
            if resident + cost <= self.budget {
                break;
            }
            if let Some(entry) = held.take_if(|entry| idle(entry)) {
                resident -= texture_bytes(&entry.texture);
            }
        }
        trim(&mut self.entries);
    }

    /// Returns the device region a target for `region` covers.
    fn cover(&self, region: Rect<i32, Device>) -> Rect<i32, Device> {
        cover_for(region, self.region, self.margin)
    }
}
