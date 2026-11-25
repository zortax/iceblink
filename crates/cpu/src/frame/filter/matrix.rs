//! A colour matrix on straight colour, and the composition of several.

use iceblink::FrameFilter;

/// A 4x5 colour matrix on straight colour, as rows and an offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ColorMatrix {
    rows: [[f32; 4]; 4],
    offset: [f32; 4],
}

impl ColorMatrix {
    /// Returns the matrix that changes nothing.
    pub(crate) fn identity() -> Self {
        Self {
            rows: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            offset: [0.0; 4],
        }
    }

    /// Reports whether the matrix changes nothing.
    pub(crate) fn is_identity(&self) -> bool {
        *self == Self::identity()
    }

    /// Returns the matrix of a per-pixel filter, or `None` for a filter that reads neighbours.
    pub(crate) fn of(filter: FrameFilter) -> Option<Self> {
        match filter {
            FrameFilter::Blur(_) | FrameFilter::DropShadow { .. } | FrameFilter::Shader { .. } => {
                None
            }
            FrameFilter::ColorMatrix(m) => Some(Self {
                rows: [
                    [m[0], m[1], m[2], m[3]],
                    [m[5], m[6], m[7], m[8]],
                    [m[10], m[11], m[12], m[13]],
                    [m[15], m[16], m[17], m[18]],
                ],
                offset: [m[4], m[9], m[14], m[19]],
            }),
            FrameFilter::Opacity(amount) => {
                let mut matrix = Self::identity();
                matrix.rows[3][3] = amount.clamp(0.0, 1.0);
                Some(matrix)
            }
        }
    }

    /// Returns the matrix that applies `self` and then `next`.
    pub(crate) fn then(self, next: Self) -> Self {
        let mut rows = [[0.0f32; 4]; 4];
        for (out, row) in rows.iter_mut().enumerate() {
            for (input, cell) in row.iter_mut().enumerate() {
                *cell = (0..4)
                    .map(|middle| next.rows[out][middle] * self.rows[middle][input])
                    .sum();
            }
        }
        let mut offset = next.offset;
        for (out, value) in offset.iter_mut().enumerate() {
            *value += (0..4)
                .map(|middle| next.rows[out][middle] * self.offset[middle])
                .sum::<f32>();
        }
        Self { rows, offset }
    }

    /// Returns the premultiplied colour `color` through the matrix, premultiplied again.
    ///
    /// The results are clamped to `[0, 1]`, and a pixel with zero alpha stays transparent.
    pub(crate) fn apply(&self, color: [f32; 4]) -> [f32; 4] {
        if self.is_identity() {
            return color;
        }
        let alpha = color[3];
        if alpha <= 0.0 {
            return [0.0; 4];
        }
        let straight = [color[0] / alpha, color[1] / alpha, color[2] / alpha, alpha];
        let mut out = [0.0f32; 4];
        for (channel, value) in out.iter_mut().enumerate() {
            let row = self.rows[channel];
            *value = (row[0] * straight[0]
                + row[1] * straight[1]
                + row[2] * straight[2]
                + row[3] * straight[3]
                + self.offset[channel])
                .clamp(0.0, 1.0);
        }
        [out[0] * out[3], out[1] * out[3], out[2] * out[3], out[3]]
    }
}
