//! A texture table of fixed-size records, allocated in ranges.
//!
//! Uploads are staged and recorded as copies when the arena is flushed. An upload that starts
//! where the staged records end joins them, so records placed one after another are copied
//! together.

use std::ops::Range;

use crate::gpu::device::Gpu;
use crate::upload::{TEXEL, TEXELS_WIDE, UploadBelt, allocate_rows, row_limit, write_run};

/// A texture table of fixed-size records with a free list, grown by reallocation.
#[derive(Debug)]
pub(crate) struct Arena {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) label: &'static str,
    /// The bytes of one record.
    pub(crate) element: u32,
    /// The bytes one record takes in the texture, a whole number of texels.
    pub(crate) stride: u32,
    /// How many records the texture holds.
    pub(crate) capacity: u32,
    /// A number that changes whenever the arena gets a new texture.
    pub(crate) generation: u64,
    /// The first record past every allocated range.
    pub(crate) tail: u32,
    /// The free ranges below the tail, sorted and merged.
    pub(crate) free: Vec<Range<u32>>,
    /// Whether an overflow was logged.
    pub(crate) overflowed: bool,
    /// The first texel of the staged records.
    staged_at: u32,
    /// The staged records, each padded to the stride.
    staged: Vec<u8>,
}

impl Arena {
    const MINIMUM_BYTES: u64 = 256;

    pub(crate) fn new(gpu: &Gpu, label: &'static str, element: u32) -> Self {
        let stride = element.div_ceil(TEXEL as u32).saturating_mul(TEXEL as u32);
        let capacity = (Self::MINIMUM_BYTES as u32 / stride).max(1);
        let (texture, view) = allocate(gpu, label, capacity, stride);
        Self {
            texture,
            view,
            label,
            element,
            stride,
            capacity,
            generation: 1,
            tail: 0,
            free: Vec::new(),
            overflowed: false,
            staged_at: 0,
            staged: Vec::new(),
        }
    }

    /// Returns the most records a texture on the device holds.
    pub(crate) fn limit(&self, gpu: &Gpu) -> u32 {
        let texels = u64::from(row_limit(gpu.device())) * u64::from(TEXELS_WIDE);
        (texels / u64::from(self.stride / TEXEL as u32)).min(u64::from(u32::MAX)) as u32
    }

    /// Returns a range of `len` records past the capacity, which is never uploaded, and logs the
    /// first overflow.
    pub(crate) fn past(&mut self, len: u32) -> Range<u32> {
        if !std::mem::replace(&mut self.overflowed, true) {
            tracing::error!(
                arena = self.label,
                capacity = self.capacity,
                asked = len,
                "an arena holds more records than a texture can; the records past the limit are \
                 not drawn"
            );
        }
        self.capacity..self.capacity.saturating_add(len)
    }

    /// Returns a range of `len` records from the free ranges or the tail, or `None` if neither
    /// has room.
    pub(crate) fn alloc(&mut self, len: u32) -> Option<Range<u32>> {
        if len == 0 {
            return Some(0..0);
        }
        if let Some(at) = self
            .free
            .iter()
            .position(|range| range.end - range.start >= len)
        {
            let range = self.free[at].clone();
            let taken = range.start..range.start + len;
            if range.end - range.start == len {
                // An ordered remove, because the order is what `free` merges by.
                self.free.remove(at);
            } else {
                self.free[at] = range.start + len..range.end;
            }
            return Some(taken);
        }
        if self.tail + len <= self.capacity {
            let taken = self.tail..self.tail + len;
            self.tail += len;
            return Some(taken);
        }
        None
    }

    /// Returns `range` to the free ranges.
    pub(crate) fn free(&mut self, range: Range<u32>) {
        // A range from `past` was never part of the arena.
        let range = range.start.min(self.capacity)..range.end.min(self.capacity);
        if range.is_empty() {
            return;
        }
        let at = self.free.partition_point(|held| held.start < range.start);
        let merges_left = at > 0 && self.free[at - 1].end == range.start;
        let merges_right = at < self.free.len() && range.end == self.free[at].start;
        match (merges_left, merges_right) {
            (true, true) => {
                self.free[at - 1].end = self.free[at].end;
                self.free.remove(at);
            }
            (true, false) => self.free[at - 1].end = range.end,
            (false, true) => self.free[at].start = range.start,
            (false, false) => self.free.insert(at, range),
        }
        // A free range that reaches the tail goes back to the tail, so a drained arena
        // allocates from zero again.
        while let Some(last) = self.free.last() {
            if last.end != self.tail {
                break;
            }
            self.tail = last.start;
            self.free.pop();
        }
    }

    /// Stages `bytes`, whole records, for the records from `start` on, and returns the bytes
    /// staged.
    ///
    /// Staged records are copied into the texture when the arena is flushed.
    pub(crate) fn upload(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        start: u32,
        bytes: &[u8],
    ) -> u64 {
        if bytes.is_empty() {
            return 0;
        }
        let element = self.element as usize;
        let stride = self.stride as usize;
        let first = start.saturating_mul(self.stride / TEXEL as u32);
        let staged_end = self.staged_at + (self.staged.len() / TEXEL) as u32;
        if self.staged.is_empty() || first != staged_end {
            self.flush(gpu, belt, encoder);
            self.staged_at = first;
        }

        let at = self.staged.len();
        let count = bytes.len() / element;
        self.staged.resize(at + count * stride, 0);
        for (index, record) in bytes.chunks_exact(element).enumerate() {
            let record_at = at + index * stride;
            self.staged[record_at..record_at + element].copy_from_slice(record);
        }
        (count * stride) as u64
    }

    /// Records the copies of the staged records into the texture.
    pub(crate) fn flush(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if self.staged.is_empty() {
            return;
        }
        gpu.uploaded(write_run(
            gpu.device(),
            belt,
            encoder,
            &self.texture,
            self.staged_at,
            &self.staged,
        ));
        self.staged.clear();
    }

    /// Replaces the texture with an empty one of at least `needed` records, up to the most a
    /// texture on the device holds, and forgets every range.
    pub(crate) fn reset_with_capacity(&mut self, gpu: &Gpu, needed: u32) {
        let bytes = (u64::from(needed) * u64::from(self.stride))
            .next_power_of_two()
            .max(Self::MINIMUM_BYTES);
        // Never past what a texture on this device can hold.
        self.capacity = ((bytes / u64::from(self.stride)) as u32).min(self.limit(gpu));
        let (texture, view) = allocate(gpu, self.label, self.capacity, self.stride);
        self.texture = texture;
        self.view = view;
        self.generation = self.generation.wrapping_add(1);
        self.tail = 0;
        self.free.clear();
        // Every record is uploaded again into the new texture.
        self.staged.clear();
    }
}

/// Allocates the texture of `capacity` records of `stride` bytes, and returns it with its view.
fn allocate(
    gpu: &Gpu,
    label: &'static str,
    capacity: u32,
    stride: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texels = capacity.saturating_mul(stride / TEXEL as u32);
    allocate_rows(gpu.device(), label, texels.div_ceil(TEXELS_WIDE).max(1))
}
