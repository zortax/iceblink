//! The picture a frame's composites leave.

/// A square picture of premultiplied colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Canvas {
    /// The width and height, in pixels.
    pub side: i32,
    pixels: Vec<[f32; 4]>,
}

impl Canvas {
    /// Returns a canvas of `side` square, every pixel the straight colour `background`.
    pub fn filled(side: i32, background: [u8; 4]) -> Self {
        let alpha = f32::from(background[3]) / 255.0;
        let pixel = [
            f32::from(background[0]) / 255.0 * alpha,
            f32::from(background[1]) / 255.0 * alpha,
            f32::from(background[2]) / 255.0 * alpha,
            alpha,
        ];
        Self {
            side,
            pixels: vec![pixel; (side * side) as usize],
        }
    }

    /// Blends the straight colour `texel` over pixel `(x, y)`, source over.
    pub fn blend(&mut self, x: i32, y: i32, texel: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.side || y >= self.side {
            return;
        }
        let alpha = f32::from(texel[3]) / 255.0;
        let pixel = &mut self.pixels[(y * self.side + x) as usize];
        for channel in 0..3 {
            let source = f32::from(texel[channel]) / 255.0 * alpha;
            pixel[channel] = source + pixel[channel] * (1.0 - alpha);
        }
        pixel[3] = alpha + pixel[3] * (1.0 - alpha);
    }

    /// Returns the premultiplied colour of pixel `(x, y)`, in bytes.
    pub fn rgba(&self, x: i32, y: i32) -> [u8; 4] {
        self.pixels[(y * self.side + x) as usize].map(|channel| (channel * 255.0).round() as u8)
    }

    /// Returns the pixel where the two canvases differ most, and the difference on its widest
    /// channel.
    pub fn difference(&self, other: &Self) -> Option<(i32, i32, u8)> {
        let mut worst: Option<(i32, i32, u8)> = None;
        for y in 0..self.side {
            for x in 0..self.side {
                let (one, two) = (self.rgba(x, y), other.rgba(x, y));
                let delta = (0..4).map(|c| one[c].abs_diff(two[c])).max().unwrap_or(0);
                if delta > 0 && worst.is_none_or(|(_, _, held)| delta > held) {
                    worst = Some((x, y, delta));
                }
            }
        }
        worst
    }

    /// Measures how this canvas agrees with `reference`, split into interior and edge pixels.
    ///
    /// A pixel is interior when the reference is uniform over its 3 by 3 neighbourhood, and an
    /// edge pixel when it is not and either canvas paints it with something besides `ground`.
    pub fn agreement(&self, reference: &Self, ground: [u8; 4]) -> Agreement {
        let mut found = Agreement::default();
        let mut sum = 0_u64;
        for y in 0..self.side {
            for x in 0..self.side {
                let (got, want) = (self.rgba(x, y), reference.rgba(x, y));
                let difference = (0..4).map(|c| got[c].abs_diff(want[c])).max().unwrap_or(0);
                let uniform = (-1..=1).all(|dy| {
                    (-1..=1).all(|dx| {
                        let (nx, ny) = (
                            (x + dx).clamp(0, self.side - 1),
                            (y + dy).clamp(0, self.side - 1),
                        );
                        reference.rgba(nx, ny) == want
                    })
                });
                if uniform {
                    found.interior += 1;
                    found.interior_worst = found.interior_worst.max(difference);
                } else if got != ground || want != ground {
                    found.edge += 1;
                    sum += u64::from(difference);
                    found.worst = found.worst.max(difference);
                }
            }
        }
        found.mean = sum as f64 / f64::from(found.edge.max(1));
        found
    }
}

/// How one canvas agrees with a reference.
#[derive(Clone, Copy, Debug, Default)]
pub struct Agreement {
    /// The interior pixels.
    pub interior: u32,
    /// The largest channel difference at an interior pixel.
    pub interior_worst: u8,
    /// The edge pixels.
    pub edge: u32,
    /// The mean of the largest channel difference at each edge pixel.
    pub mean: f64,
    /// The largest channel difference at an edge pixel.
    pub worst: u8,
}
