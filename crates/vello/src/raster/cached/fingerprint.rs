//! What an encoding depends on.

use iceblink::{FramePaint, FramePath};

/// What the encoding of a path depends on.
///
/// Two paths that agree on all of it share one encoding, wherever each is placed and clipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Fingerprint {
    /// The address of the path's allocation.
    path: usize,
    /// The contents of the fill and stroke paints, folded.
    paint: u64,
    /// The fill rule and the stroke style, folded.
    style: u64,
    /// The bits of the brush coefficients.
    brush: [u64; 6],
}

impl Fingerprint {
    /// Returns the fingerprint of `path` drawn with the `fill` and `stroke` paints.
    pub(crate) fn of(
        path: &FramePath,
        fill: Option<&FramePaint>,
        stroke: Option<&FramePaint>,
    ) -> Self {
        let mut paint = Fold::new();
        paint.paint(fill);
        paint.paint(stroke);

        let mut style = Fold::new();
        style.add(u64::from(path.fill.map_or(u8::MAX, |fill| fill.rule.0)));
        if let Some(stroke) = &path.stroke {
            style.stroke(&stroke.style);
        }

        Self {
            path: std::sync::Arc::as_ptr(&path.path) as usize,
            paint: paint.0,
            style: style.0,
            brush: path.brush.as_coeffs().map(f64::to_bits),
        }
    }
}

/// An FNV-1a fold of 64-bit words.
struct Fold(u64);

impl Fold {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn add(&mut self, value: u64) {
        self.0 ^= value;
        self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
    }

    fn floats(&mut self, values: &[f32]) {
        for value in values {
            self.add(u64::from(value.to_bits()));
        }
    }

    /// Folds in every number of `paint`, or a marker for no paint.
    fn paint(&mut self, paint: Option<&FramePaint>) {
        match paint {
            None | Some(FramePaint::None) => self.add(0),
            Some(FramePaint::Solid(color)) => {
                self.add(1);
                self.floats(&color.0);
            }
            Some(FramePaint::Sides(colors)) => {
                self.add(2);
                for color in colors {
                    self.floats(&color.0);
                }
            }
            Some(FramePaint::Gradient(gradient)) => {
                self.add(3);
                match gradient.shape {
                    iceblink::GradientShape::Linear { start, end } => {
                        self.add(0);
                        self.floats(&start);
                        self.floats(&end);
                    }
                    iceblink::GradientShape::Radial { center, radius } => {
                        self.add(1);
                        self.floats(&center);
                        self.floats(&radius);
                    }
                    iceblink::GradientShape::Conic { center, angle } => {
                        self.add(2);
                        self.floats(&center);
                        self.floats(&[angle]);
                    }
                }
                self.add(u64::from(gradient.encoding.0));
                self.add(u64::from(gradient.repeat));
                for stop in gradient.stops.iter() {
                    self.floats(&stop.color);
                    self.floats(&[stop.offset]);
                }
            }
        }
    }

    /// Folds in every number the outline of a stroke depends on.
    // The width alone is not enough: a dashed line restyled solid moves no geometry.
    fn stroke(&mut self, stroke: &kurbo::Stroke) {
        self.add(stroke.width.to_bits());
        self.add(stroke.miter_limit.to_bits());
        self.add(stroke.dash_offset.to_bits());
        self.add(stroke.join as u64);
        self.add(stroke.start_cap as u64);
        self.add(stroke.end_cap as u64);
        for dash in &stroke.dash_pattern {
            self.add(dash.to_bits());
        }
    }
}
