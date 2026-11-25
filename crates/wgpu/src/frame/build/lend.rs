//! The targets a plan lends from the pool, and what it knows about each target.

use iceblink::geom::{Device, Rect, Size};

use crate::frame::build::PlanBuilder;
use crate::frame::target::TargetRef;
use crate::target::group_pool::{GroupPool, GroupSlot};
use crate::target::scale::TargetScale;

impl PlanBuilder<'_> {
    /// Returns the extent `target` is allocated at.
    pub(crate) fn extent_of(&self, target: TargetRef) -> Size<i32, Device> {
        match target {
            TargetRef::Composed => self.composed_extent,
            TargetRef::Pool(slot) => slot.allocated(),
            TargetRef::MarksPage(_) => self.mark_extent,
        }
    }

    pub(crate) fn format_of(&self, target: TargetRef) -> wgpu::TextureFormat {
        match target.slot() {
            Some(slot) => slot.format(),
            None => self.composed_format,
        }
    }

    /// Lends a slot of the pool's format that covers `region`.
    ///
    /// The slot is at `scale`, or at half scale when the pool's budget is full.
    pub(crate) fn acquire(
        &mut self,
        scale: TargetScale,
        region: Rect<i32, Device>,
    ) -> Option<GroupSlot> {
        self.acquire_in(scale, GroupPool::FORMAT, region)
    }

    /// Lends a slot for a group's content that covers `region`.
    ///
    /// The slot is at `scale`, or at half scale when the pool's budget is full. A `plain` group
    /// may get the composed format.
    pub(crate) fn acquire_group(
        &mut self,
        scale: TargetScale,
        region: Rect<i32, Device>,
        plain: bool,
    ) -> Option<GroupSlot> {
        let format = self.pool.format_for(plain, self.composed_format);
        self.acquire_in(scale, format, region)
    }

    /// Lends a slot at exactly the scale and format of `like` that covers `region`.
    pub(crate) fn acquire_like(
        &mut self,
        like: TargetRef,
        region: Rect<i32, Device>,
    ) -> Option<GroupSlot> {
        let slot = self.pool.acquire_exact_covering(
            self.gpu,
            like.scale(),
            self.format_of(like),
            region,
        )?;
        Some(self.lend(slot))
    }

    /// Reports whether no pass has written `slot` since it was lent.
    pub(crate) fn is_unwritten(&self, slot: GroupSlot) -> bool {
        self.unwritten.contains(&slot)
    }

    /// Returns `slot` to the pool.
    pub(crate) fn release(&mut self, slot: GroupSlot) {
        self.pool.release(slot);
        if let Some(at) = self.lent.iter().position(|held| *held == slot) {
            self.lent.remove(at);
        }
        self.unwritten.retain(|held| *held != slot);
    }

    fn acquire_in(
        &mut self,
        scale: TargetScale,
        format: wgpu::TextureFormat,
        region: Rect<i32, Device>,
    ) -> Option<GroupSlot> {
        let slot = self
            .pool
            .acquire_covering(self.gpu, scale, format, region)?;
        Some(self.lend(slot))
    }

    fn lend(&mut self, slot: GroupSlot) -> GroupSlot {
        self.lent.push(slot);
        self.unwritten.push(slot);
        slot
    }
}
