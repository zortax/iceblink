//! An owned target at a device position.

use iceblink::geom::{Device, Point, Rect, Size};

use crate::target::surface::{Surface, TexelsMut};
use crate::target::texture::{Texels, Texture};
use crate::target::to_byte;

/// How many bits a canvas holds per channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Precision {
    /// Eight bits, rounded after every write.
    Byte,
    /// A float.
    Float,
}

/// A transparent-initialised target covering a device rectangle, or one texel per two by two
/// device pixels of it.
#[derive(Clone, Debug)]
pub(crate) struct Canvas {
    /// The device pixel the first texel holds.
    origin: Point<i32, Device>,
    width: usize,
    height: usize,
    /// Whether one texel covers two by two device pixels.
    half: bool,
    bytes: Vec<[u8; 4]>,
    floats: Vec<[f32; 4]>,
    precision: Precision,
}

impl Canvas {
    /// Returns a transparent canvas of `precision` over `region`.
    pub(crate) fn new(region: Rect<i32, Device>, precision: Precision) -> Self {
        let mut canvas = Self {
            origin: region.origin,
            width: 0,
            height: 0,
            half: false,
            bytes: Vec::new(),
            floats: Vec::new(),
            precision,
        };
        canvas.reset(region, precision, false);
        canvas
    }

    /// Makes the canvas a transparent one over `region`, at half resolution when `half` is set,
    /// keeping its allocation.
    ///
    /// A half-resolution canvas starts on an even device pixel at or before the region.
    pub(crate) fn reset(&mut self, region: Rect<i32, Device>, precision: Precision, half: bool) {
        let (origin, width, height) = if half {
            let origin = Point::new(
                region.origin.x.div_euclid(2) * 2,
                region.origin.y.div_euclid(2) * 2,
            );
            let right = region.origin.x + region.size.width.max(0);
            let bottom = region.origin.y + region.size.height.max(0);
            (
                origin,
                ((right - origin.x + 1) / 2).max(1) as usize,
                ((bottom - origin.y + 1) / 2).max(1) as usize,
            )
        } else {
            (
                region.origin,
                region.size.width.max(0) as usize,
                region.size.height.max(0) as usize,
            )
        };
        self.origin = origin;
        self.width = width;
        self.height = height;
        self.half = half;
        self.precision = precision;
        let count = width * height;
        match precision {
            Precision::Byte => {
                self.bytes.clear();
                self.bytes.resize(count, [0; 4]);
            }
            Precision::Float => {
                self.floats.clear();
                self.floats.resize(count, [0.0; 4]);
            }
        }
    }

    /// Returns the canvas as a drawing surface.
    pub(crate) fn surface(&mut self) -> Surface<'_> {
        let texels = match self.precision {
            Precision::Byte => TexelsMut::Byte(&mut self.bytes),
            Precision::Float => TexelsMut::Float(&mut self.floats),
        };
        Surface::new(self.origin, self.width, self.height, texels)
    }

    /// Returns the canvas as a texture to sample.
    pub(crate) fn texture(&self) -> Texture<'_> {
        let texels = match self.precision {
            Precision::Byte => Texels::Byte(&self.bytes),
            Precision::Float => Texels::Float(&self.floats),
        };
        Texture::new(self.origin, self.width, self.height, self.half, texels)
    }

    /// Returns the device pixels the canvas covers.
    pub(crate) fn region(&self) -> Rect<i32, Device> {
        let scale = if self.half { 2 } else { 1 };
        Rect::new(
            self.origin,
            Size::new(self.width as i32 * scale, self.height as i32 * scale),
        )
    }

    /// Returns the bytes the canvas holds.
    pub(crate) fn bytes(&self) -> u64 {
        (self.bytes.capacity() * 4 + self.floats.capacity() * 16) as u64
    }

    /// Returns the canvas's size in texels.
    pub(crate) fn texel_size(&self) -> [usize; 2] {
        [self.width, self.height]
    }

    /// Stores `value` in texel `(x, y)`, rounded to eight bits on a byte canvas.
    pub(crate) fn store(&mut self, x: usize, y: usize, value: [f32; 4]) {
        let at = y * self.width + x;
        match self.precision {
            Precision::Byte => self.bytes[at] = value.map(to_byte),
            Precision::Float => self.floats[at] = value,
        }
    }
}
