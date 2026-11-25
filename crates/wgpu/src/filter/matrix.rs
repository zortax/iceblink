//! A colour matrix on straight colour, and the composition of several.

use iceblink::FrameFilter;

/// A matrix and an offset applied to straight RGBA colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ColorMatrix {
    rows: [[f32; 4]; 4],
    offset: [f32; 4],
}

impl ColorMatrix {
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
            FrameFilter::Opacity(amount) => Some(Self::opacity(amount)),
        }
    }

    /// Returns the matrix that scales alpha by `amount`, clamped to between 0 and 1.
    pub(crate) fn opacity(amount: f32) -> Self {
        let mut matrix = Self::identity();
        matrix.rows[3][3] = amount.clamp(0.0, 1.0);
        matrix
    }

    /// Returns the matrix that applies this one and then `next`.
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

    /// Returns the four columns of the matrix, then the offset.
    pub(crate) fn columns(&self) -> [[f32; 4]; 5] {
        let mut columns = [[0.0f32; 4]; 5];
        for (input, column) in columns.iter_mut().take(4).enumerate() {
            for (output, cell) in column.iter_mut().enumerate() {
                *cell = self.rows[output][input];
            }
        }
        columns[4] = self.offset;
        columns
    }
}
