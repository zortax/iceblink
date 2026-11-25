//! An image of premultiplied eight-bit RGBA texels.

/// An image of premultiplied, gamma-encoded sRGB texels, eight bits per channel, row by row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pixmap {
    width: u32,
    height: u32,
    texels: Vec<[u8; 4]>,
}

impl Pixmap {
    /// Returns a transparent pixmap of `width` by `height`.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            texels: vec![[0; 4]; width as usize * height as usize],
        }
    }

    /// Returns the width in texels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height in texels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Returns the texels, row by row, in the order red, green, blue, alpha.
    pub fn data(&self) -> &[u8] {
        self.texels.as_flattened()
    }

    /// Returns the texel at `(x, y)`, or `None` outside the pixmap.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        (x < self.width && y < self.height)
            .then(|| self.texels[y as usize * self.width as usize + x as usize])
    }

    /// Returns the texels as four-byte arrays.
    pub(crate) fn texels(&self) -> &[[u8; 4]] {
        &self.texels
    }

    /// Returns the texels as four-byte arrays, mutably.
    pub(crate) fn texels_mut(&mut self) -> &mut [[u8; 4]] {
        &mut self.texels
    }

    /// Resizes the pixmap, keeping the texels both sizes share and clearing the rest.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        if (width, height) == (self.width, self.height) {
            return;
        }
        let mut resized = Self::new(width, height);
        let columns = width.min(self.width) as usize;
        for y in 0..height.min(self.height) as usize {
            let from = y * self.width as usize;
            let to = y * width as usize;
            resized.texels[to..to + columns].copy_from_slice(&self.texels[from..from + columns]);
        }
        *self = resized;
    }

    /// Returns the bytes the texels take.
    pub(crate) fn bytes(&self) -> u64 {
        self.texels.len() as u64 * 4
    }
}
