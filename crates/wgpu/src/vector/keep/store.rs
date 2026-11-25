//! Kept rasters, by pass key and region size.

use std::sync::Arc;

use iceblink::PassKey;
use rustc_hash::FxHashMap;

/// One kept raster.
pub(super) struct Raster {
    pub(super) texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    /// The outlines the key is derived from, held so that no other outline reuses their addresses.
    _outlines: Vec<Arc<kurbo::BezPath>>,
    /// The last frame that composited it.
    used: u64,
}

impl Raster {
    fn bytes(&self) -> u64 {
        u64::from(self.texture.width()) * u64::from(self.texture.height()) * 4
    }
}

/// Kept rasters, by pass key and region size.
///
/// Once the rasters hold more than the byte cap, those composited longest ago are dropped, and
/// never one composited in the current frame.
#[derive(Default)]
pub(super) struct Store {
    rasters: Vec<Option<Raster>>,
    by_key: FxHashMap<(PassKey, [i32; 2]), u32>,
    free: Vec<u32>,
    bytes: u64,
    frame: u64,
}

impl Store {
    /// The default byte cap.
    pub(super) const BYTE_CAP: u64 = 64 << 20;

    /// Starts a frame.
    pub(super) fn begin(&mut self) {
        self.frame += 1;
    }

    /// Returns the raster kept for `key` at `size`, marked as composited in this frame.
    pub(super) fn find(&mut self, key: PassKey, size: [i32; 2]) -> Option<u32> {
        let index = *self.by_key.get(&(key, size))?;
        let raster = self.rasters[index as usize].as_mut()?;
        raster.used = self.frame;
        Some(index)
    }

    /// Reports whether a raster is kept for `key` at `size`.
    pub(super) fn holds(&self, key: PassKey, size: [i32; 2]) -> bool {
        self.by_key.contains_key(&(key, size))
    }

    /// Returns the raster at `index`.
    pub(super) fn get(&self, index: u32) -> Option<&Raster> {
        self.rasters.get(index as usize)?.as_ref()
    }

    /// Keeps `texture` as the raster of `key` at `size`, then drops old rasters past `cap`.
    pub(super) fn insert(
        &mut self,
        key: PassKey,
        size: [i32; 2],
        texture: wgpu::Texture,
        outlines: Vec<Arc<kurbo::BezPath>>,
        cap: u64,
    ) {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let raster = Raster {
            texture,
            view,
            _outlines: outlines,
            used: self.frame,
        };
        self.bytes += raster.bytes();
        let index = match self.free.pop() {
            Some(index) => {
                self.rasters[index as usize] = Some(raster);
                index
            }
            None => {
                self.rasters.push(Some(raster));
                (self.rasters.len() - 1) as u32
            }
        };
        self.by_key.insert((key, size), index);
        self.trim(cap);
    }

    /// Drops the rasters composited longest ago until the rest fit in `cap`.
    fn trim(&mut self, cap: u64) {
        while self.bytes > cap {
            let oldest = self
                .by_key
                .iter()
                .filter_map(|(&entry, &index)| {
                    let raster = self.rasters[index as usize].as_ref()?;
                    (raster.used < self.frame).then_some((raster.used, entry, index))
                })
                .min_by_key(|&(used, _, _)| used);
            let Some((_, entry, index)) = oldest else {
                return;
            };
            self.remove(entry, index);
        }
    }

    fn remove(&mut self, entry: (PassKey, [i32; 2]), index: u32) {
        self.by_key.remove(&entry);
        if let Some(raster) = self.rasters[index as usize].take() {
            self.bytes -= raster.bytes();
        }
        self.free.push(index);
    }

    /// Drops every raster not composited in the current frame, and returns the bytes freed.
    pub(super) fn release(&mut self) -> u64 {
        let before = self.bytes;
        let stale: Vec<_> = self
            .by_key
            .iter()
            .filter(|&(_, &index)| {
                self.rasters[index as usize]
                    .as_ref()
                    .is_some_and(|raster| raster.used < self.frame)
            })
            .map(|(&entry, &index)| (entry, index))
            .collect();
        for (entry, index) in stale {
            self.remove(entry, index);
        }
        before - self.bytes
    }

    /// Returns how many rasters are kept.
    pub(super) fn len(&self) -> usize {
        self.by_key.len()
    }

    /// Returns the bytes the kept rasters hold.
    pub(super) fn bytes(&self) -> u64 {
        self.bytes
    }
}
