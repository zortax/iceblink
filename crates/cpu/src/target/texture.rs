//! A read-only view of a target, sampled as the reference renderer's filtering sampler does.

use iceblink::geom::{Device, Point};

use crate::math::Floor as _;
use crate::math::vec::mix4;
use crate::target::from_byte;

/// The texels of a texture.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Texels<'a> {
    /// Eight bits per channel.
    Byte(&'a [[u8; 4]]),
    /// A float per channel.
    Float(&'a [[f32; 4]]),
}

/// A target to read: texels at a device position, one per device pixel or one per two by two.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Texture<'a> {
    origin: Point<i32, Device>,
    width: usize,
    height: usize,
    half: bool,
    texels: Texels<'a>,
}

impl<'a> Texture<'a> {
    /// Returns a texture over `texels`, which hold `height` rows of `width` texels.
    pub(crate) fn new(
        origin: Point<i32, Device>,
        width: usize,
        height: usize,
        half: bool,
        texels: Texels<'a>,
    ) -> Self {
        Self {
            origin,
            width,
            height,
            half,
            texels,
        }
    }

    /// Returns how many texels one device pixel covers, along each axis.
    pub(crate) fn scale(&self) -> f32 {
        if self.half { 0.5 } else { 1.0 }
    }

    /// Returns the device pixel the first texel holds.
    pub(crate) fn origin(&self) -> Point<i32, Device> {
        self.origin
    }

    /// Returns the texel at `(x, y)`, clamped to the edge.
    pub(crate) fn load(&self, x: i32, y: i32) -> [f32; 4] {
        if self.width == 0 || self.height == 0 {
            return [0.0; 4];
        }
        let x = x.clamp(0, self.width as i32 - 1) as usize;
        let y = y.clamp(0, self.height as i32 - 1) as usize;
        let at = y * self.width + x;
        match self.texels {
            Texels::Byte(texels) => texels[at].map(from_byte),
            Texels::Float(texels) => texels[at],
        }
    }

    /// Returns the texture sampled bilinearly at `position`, in texels.
    pub(crate) fn bilinear(&self, position: [f32; 2]) -> [f32; 4] {
        let at = [position[0] - 0.5, position[1] - 0.5];
        let base = [at[0].floored(), at[1].floored()];
        let weight = [at[0] - base[0], at[1] - base[1]];
        let [x, y] = [base[0] as i32, base[1] as i32];
        // A position on a texel centre reads that texel alone.
        let row = |y: i32| {
            if weight[0] == 0.0 {
                self.load(x, y)
            } else {
                mix4(self.load(x, y), self.load(x + 1, y), weight[0])
            }
        };
        if weight[1] == 0.0 {
            return row(y);
        }
        mix4(row(y), row(y + 1), weight[1])
    }

    /// Returns the texture sampled bilinearly at the device point `device`.
    pub(crate) fn sample(&self, device: [f32; 2]) -> [f32; 4] {
        let scale = self.scale();
        self.bilinear([
            (device[0] - self.origin.x as f32) * scale,
            (device[1] - self.origin.y as f32) * scale,
        ])
    }

    /// Returns the texture sampled at the device point `device`, fading to transparent over the
    /// device pixel past its edges.
    pub(crate) fn sample_transparent(&self, device: [f32; 2]) -> [f32; 4] {
        let scale = self.scale();
        let at = [
            device[0] - self.origin.x as f32,
            device[1] - self.origin.y as f32,
        ];
        let inside = |at: f32, texels: usize| {
            let size = texels as f32 / scale;
            (at + 0.5).clamp(0.0, 1.0) * (size + 0.5 - at).clamp(0.0, 1.0)
        };
        let weight = inside(at[0], self.width) * inside(at[1], self.height);
        if weight <= 0.0 {
            return [0.0; 4];
        }
        self.sample(device).map(|channel| channel * weight)
    }

    /// Returns the texel that holds the device pixel `(x, y)` of a full-resolution texture.
    pub(crate) fn at_pixel(&self, x: i32, y: i32) -> [f32; 4] {
        self.load(x - self.origin.x, y - self.origin.y)
    }
}
