//! Pooled targets for groups, filter steps and backdrop captures.
//!
//! Targets are sized in classes and reused across frames, so a group that moves or changes size
//! rarely allocates. When the memory budget is full, a target can be lent at half resolution. A
//! target that stays unlent for a stretch of frames is freed.

mod cover;
mod lend;
mod precision;
mod slot;

use iceblink::geom::{Device, Size};

use crate::target::group_pool::cover::{
    allocated_for, even_margin, poison_margin, whole_allocated, whole_cover,
};
use crate::target::scale::TargetScale;

pub use self::precision::GroupPrecision;
pub(crate) use self::slot::GroupSlot;

/// A target the pool holds.
#[derive(Debug)]
struct Entry {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    extent: Size<i32, Device>,
    scale: TargetScale,
    format: wgpu::TextureFormat,
    /// Whether the target is lent out.
    lent: bool,
    /// Whether the target was lent since the last `release_all`.
    touched: bool,
    /// How many frames in a row the target stayed unlent.
    idle: u32,
}

/// The pool of group targets a renderer lends to groups, filter steps and backdrops.
#[derive(Debug)]
pub struct GroupPool {
    /// The size of the output the targets serve.
    region: Size<i32, Device>,
    /// How far every target's origin is moved up and left, in device pixels.
    margin: i32,
    entries: Vec<Option<Entry>>,
    budget: u64,
    /// Whether the budget follows the output's size.
    scaled: bool,
    degraded: u32,
    peak: u32,
    leases: u64,
    allocations: u64,
    precision: GroupPrecision,
}

impl GroupPool {
    /// The half-float format of targets that eight bits per channel do not suit.
    pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    /// The smallest budget of a pool whose budget follows the output's size.
    pub(crate) const BUDGET: u64 = 256 * 1024 * 1024;

    /// How many output-sized targets the budget of a pool that follows the output's size holds.
    pub(crate) const SURFACE_TARGETS: u64 = 6;

    /// The environment variable that holds a margin that moves every target's origin up and left.
    pub(crate) const POISON_ORIGIN: &str = "ICEBLINK_POISON_TARGET_ORIGIN";

    /// How many device pixels a target extends past its region on each side.
    pub(crate) const GUARD: i32 = 2;

    /// How many frames a target may stay unlent before it is freed.
    pub(crate) const IDLE_FRAMES: u32 = 60;

    const USAGE: wgpu::TextureUsages = wgpu::TextureUsages::RENDER_ATTACHMENT
        .union(wgpu::TextureUsages::TEXTURE_BINDING)
        .union(wgpu::TextureUsages::COPY_DST)
        .union(wgpu::TextureUsages::COPY_SRC);

    /// Returns an empty pool for an output of `region`, with a budget of `budget` bytes.
    pub(crate) fn new(region: Size<i32, Device>, budget: u64) -> Self {
        Self {
            region: region.non_negative(),
            margin: poison_margin(),
            entries: Vec::new(),
            budget,
            scaled: false,
            degraded: 0,
            peak: 0,
            leases: 0,
            allocations: 0,
            precision: GroupPrecision::Compact,
        }
    }

    /// Returns an empty pool for an output of `region`, whose budget follows the output's size.
    pub(crate) fn scaled(region: Size<i32, Device>) -> Self {
        let mut pool = Self::new(region, Self::budget_for(region));
        pool.scaled = true;
        pool
    }

    /// Returns an empty pool with this pool's region, margin, budget and precision.
    pub(crate) fn emptied(&self) -> Self {
        Self {
            margin: self.margin,
            scaled: self.scaled,
            precision: self.precision,
            ..Self::new(self.region, self.budget)
        }
    }

    /// Returns the budget of a pool that follows an output of `region`.
    ///
    /// The budget holds [`SURFACE_TARGETS`](Self::SURFACE_TARGETS) targets of the output's size,
    /// and is at least [`BUDGET`](Self::BUDGET).
    pub(crate) fn budget_for(region: Size<i32, Device>) -> u64 {
        let full = allocated_for(region.non_negative(), TargetScale::Full);
        let one = u64::from(Self::FORMAT.block_copy_size(None).unwrap_or(8))
            * full.width.max(1) as u64
            * full.height.max(1) as u64;
        Self::BUDGET.max(Self::SURFACE_TARGETS * one)
    }

    /// Fits the pool to an output of `region`, and frees the targets larger than such an output
    /// needs.
    pub(crate) fn resize(&mut self, region: Size<i32, Device>) {
        let region = region.non_negative();
        self.region = region;
        let margin = self.margin;
        for held in &mut self.entries {
            if held.as_ref().is_some_and(|entry| {
                let largest = whole_allocated(region, margin, entry.scale);
                entry.extent.width > largest.width || entry.extent.height > largest.height
            }) {
                *held = None;
            }
        }
        trim(&mut self.entries);
        if self.scaled {
            self.budget = Self::budget_for(region);
        }
    }

    /// Returns the target of `slot` to the pool.
    pub(crate) fn release(&mut self, slot: GroupSlot) {
        if let Some(Some(entry)) = self.entries.get_mut(slot.index) {
            entry.lent = false;
        }
    }

    /// Returns every target to the pool at the end of a frame.
    ///
    /// Then the targets unlent for more than [`IDLE_FRAMES`](Self::IDLE_FRAMES) frames are freed,
    /// and the longest unlent targets, the largest first among equals, until the rest fit the
    /// budget.
    pub(crate) fn release_all(&mut self) {
        for held in &mut self.entries {
            let Some(entry) = held else {
                continue;
            };
            entry.idle = if entry.touched { 0 } else { entry.idle + 1 };
            entry.lent = false;
            entry.touched = false;
            if entry.idle > Self::IDLE_FRAMES {
                *held = None;
            }
        }
        let mut resident = self.bytes();
        while resident > self.budget {
            let Some((index, bytes)) = self
                .entries
                .iter()
                .enumerate()
                .filter_map(|(index, held)| {
                    let entry = held.as_ref()?;
                    Some((index, entry.idle, texture_bytes(&entry.texture)))
                })
                .max_by_key(|&(_, idle, bytes)| (idle, bytes))
                .map(|(index, _, bytes)| (index, bytes))
            else {
                break;
            };
            self.entries[index] = None;
            resident -= bytes;
        }
        trim(&mut self.entries);
    }

    pub(crate) fn view(&self, slot: GroupSlot) -> &wgpu::TextureView {
        &self.entry(slot).view
    }

    pub(crate) fn texture(&self, slot: GroupSlot) -> &wgpu::Texture {
        &self.entry(slot).texture
    }

    fn entry(&self, slot: GroupSlot) -> &Entry {
        self.entries[slot.index]
            .as_ref()
            .expect("a slot of this frame names a resident target")
    }

    /// Returns the format of a group target for content composed in `composed`.
    ///
    /// A `plain` group, one composited by plain source-over, keeps an eight-bit `composed` format at
    /// compact precision. Every other group is held in [`FORMAT`](Self::FORMAT).
    pub(crate) fn format_for(
        &self,
        plain: bool,
        composed: wgpu::TextureFormat,
    ) -> wgpu::TextureFormat {
        use wgpu::TextureFormat::{Bgra8Unorm, Rgba8Unorm};
        match composed {
            Bgra8Unorm | Rgba8Unorm if plain && self.precision == GroupPrecision::Compact => {
                composed
            }
            _ => Self::FORMAT,
        }
    }

    /// Returns the precision targets are held at.
    pub fn precision(&self) -> GroupPrecision {
        self.precision
    }

    pub(crate) fn set_precision(&mut self, precision: GroupPrecision) {
        self.precision = precision;
    }

    /// Moves every target's origin up and left by `margin` device pixels, made even, and frees the
    /// targets held.
    pub(crate) fn set_poison_margin(&mut self, margin: i32) {
        self.margin = even_margin(margin);
        self.entries.clear();
    }

    /// Returns the device region every target covers, the output plus the guard.
    pub fn covered(&self) -> Size<i32, Device> {
        whole_cover(self.region, self.margin).size
    }

    /// Returns how many targets the pool has allocated since it was made.
    pub fn allocations(&self) -> u64 {
        self.allocations
    }

    /// Returns the memory budget, in bytes.
    pub fn budget(&self) -> u64 {
        self.budget
    }

    /// Returns the bytes the pool's targets hold.
    pub fn bytes(&self) -> u64 {
        self.entries
            .iter()
            .flatten()
            .map(|entry| texture_bytes(&entry.texture))
            .sum()
    }

    /// Returns how many targets are lent out.
    pub fn lent(&self) -> u32 {
        self.entries
            .iter()
            .flatten()
            .filter(|entry| entry.lent)
            .count() as u32
    }

    /// Returns the bytes of the targets lent out.
    pub fn lent_bytes(&self) -> u64 {
        self.entries
            .iter()
            .flatten()
            .filter(|entry| entry.lent)
            .map(|entry| texture_bytes(&entry.texture))
            .sum()
    }

    /// Returns how many leases the pool granted since it was made.
    pub fn leases(&self) -> u64 {
        self.leases
    }

    /// Frees every target unless one is lent, and returns the bytes freed.
    pub(crate) fn release_unused(&mut self) -> u64 {
        if self.lent() > 0 {
            return 0;
        }
        let before = self.bytes();
        self.entries.clear();
        before
    }

    /// Returns the most targets lent at once.
    pub fn peak(&self) -> u32 {
        self.peak
    }

    /// Returns how many leases were held at half resolution to stay within the budget.
    pub fn degraded(&self) -> u32 {
        self.degraded
    }
}

/// Drops the empty slots at the end of `entries`.
fn trim(entries: &mut Vec<Option<Entry>>) {
    while entries.last().is_some_and(Option::is_none) {
        entries.pop();
    }
}

fn texture_bytes(texture: &wgpu::Texture) -> u64 {
    u64::from(texture.format().block_copy_size(None).unwrap_or(8))
        * u64::from(texture.width())
        * u64::from(texture.height())
}
