//! Reading a target back to the host, for tests and diagnostics.

mod renderer;
mod rows;

use iceblink::geom::{Device, Size};

use crate::gpu::device::Gpu;

pub(crate) use self::rows::{Staging, read, read_bands};

/// Texels read back from a target, four bytes each and row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pixels {
    bytes: Vec<u8>,
    size: Size<i32, Device>,
    bgra: bool,
}

impl Pixels {
    /// Returns an empty readback.
    pub fn nothing() -> Self {
        Self {
            bytes: Vec::new(),
            size: Size::new(0, 0),
            bgra: false,
        }
    }

    /// Reads every texel of `texture` back, waiting for the device.
    ///
    /// The texture needs `COPY_SRC` usage and a format of four bytes per texel.
    pub fn of_texture(gpu: &Gpu, texture: &wgpu::Texture) -> Self {
        let size = Size::new(texture.width() as i32, texture.height() as i32);
        read(gpu, texture, texture.format(), size)
    }

    /// Reports whether the readback holds texels of `size` in the channel order `bgra` says.
    pub fn holds(&self, size: Size<i32, Device>, bgra: bool) -> bool {
        self.size == size && self.bgra == bgra && !self.bytes.is_empty()
    }

    /// Makes room for texels of `size` in the channel order `bgra` says, keeping the texels when
    /// both are unchanged.
    fn resize(&mut self, size: Size<i32, Device>, bgra: bool) {
        if self.size == size && self.bgra == bgra {
            return;
        }
        self.size = size;
        self.bgra = bgra;
        self.bytes.clear();
        self.bytes
            .resize((size.width.max(0) * size.height.max(0) * 4) as usize, 0);
    }

    /// Returns the texel at `(x, y)` as red, green, blue and alpha.
    ///
    /// # Panics
    ///
    /// Panics when `(x, y)` lies outside the extent read.
    pub fn rgba(&self, x: i32, y: i32) -> [u8; 4] {
        assert!(
            x >= 0 && y >= 0 && x < self.size.width && y < self.size.height,
            "({x}, {y}) is outside the {:?} that was read",
            self.size
        );
        let offset = ((y * self.size.width + x) * 4) as usize;
        let raw = [
            self.bytes[offset],
            self.bytes[offset + 1],
            self.bytes[offset + 2],
            self.bytes[offset + 3],
        ];
        if self.bgra {
            [raw[2], raw[1], raw[0], raw[3]]
        } else {
            raw
        }
    }

    /// Returns the texels as read, in the target's channel order.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Reports whether the bytes are blue, green, red and alpha.
    pub fn is_bgra(&self) -> bool {
        self.bgra
    }

    /// Returns the extent read.
    pub fn size(&self) -> Size<i32, Device> {
        self.size
    }

    /// Returns the largest difference of any channel of any texel between two readbacks.
    ///
    /// # Panics
    ///
    /// Panics when the readbacks have different extents.
    pub fn max_difference(&self, other: &Self) -> u8 {
        assert_eq!(
            self.size, other.size,
            "two readbacks of different extents cannot be compared"
        );
        let mut worst = 0;
        for y in 0..self.size.height {
            for x in 0..self.size.width {
                let (left, right) = (self.rgba(x, y), other.rgba(x, y));
                for channel in 0..4 {
                    worst = worst.max(left[channel].abs_diff(right[channel]));
                }
            }
        }
        worst
    }
}
