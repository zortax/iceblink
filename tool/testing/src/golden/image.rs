//! An image of 8-bit RGBA pixels.

/// An image of 8-bit pixels, row by row, four bytes each in the order red, green, blue, alpha.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RgbaImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl RgbaImage {
    /// Returns a transparent image of `width` by `height`.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width as usize * height as usize * 4],
        }
    }

    /// Returns an image over `pixels`.
    ///
    /// # Panics
    ///
    /// Panics if `pixels` does not hold four bytes for each of `width × height` pixels.
    pub fn from_pixels(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        assert_eq!(
            pixels.len(),
            width as usize * height as usize * 4,
            "{width}x{height} pixels need four bytes each"
        );
        Self {
            width,
            height,
            pixels,
        }
    }

    /// Returns an image over `texels` of four bytes each, in the order blue, green, red, alpha if
    /// `bgra` is set, and red, green, blue, alpha otherwise.
    ///
    /// # Panics
    ///
    /// Panics if `texels` does not hold four bytes for each of `width × height` pixels.
    pub fn from_texels(width: u32, height: u32, texels: &[u8], bgra: bool) -> Self {
        let mut pixels = texels.to_vec();
        if bgra {
            for pixel in pixels.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
        }
        Self::from_pixels(width, height, pixels)
    }

    /// Returns the width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Returns the bytes, row by row.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Returns the pixel at `(x, y)`.
    ///
    /// # Panics
    ///
    /// Panics if the pixel lies outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let at = self.offset(x, y);
        [
            self.pixels[at],
            self.pixels[at + 1],
            self.pixels[at + 2],
            self.pixels[at + 3],
        ]
    }

    /// Sets the pixel at `(x, y)`.
    ///
    /// # Panics
    ///
    /// Panics if the pixel lies outside the image.
    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        let at = self.offset(x, y);
        self.pixels[at..at + 4].copy_from_slice(&rgba);
    }

    fn offset(&self, x: u32, y: u32) -> usize {
        assert!(
            x < self.width && y < self.height,
            "({x}, {y}) is outside the {}x{} image",
            self.width,
            self.height
        );
        (y as usize * self.width as usize + x as usize) * 4
    }
}
