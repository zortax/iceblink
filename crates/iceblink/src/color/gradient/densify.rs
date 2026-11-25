//! Densification of gradient ramps into sRGB stops.

// Segments are subdivided adaptively, so a nearly straight ramp gains no stops and a ramp across
// the hue circle gains as many as it needs.

use crate::color::gradient::GradientStop;
use crate::color::interpolate::{Interpolation, interpolate};
use crate::color::space::ColorSpace;
use crate::color::value::Color;

/// The default per-channel error bound of [`densify`], half a step of an eight-bit channel.
// Half a step keeps the error plus the rounding to eight bits within one value.
pub const DEFAULT_TOLERANCE: f32 = 0.5 / 255.0;

/// The most times a segment is halved.
// A channel driven far out of the sRGB gamut can reach its limit over a thousandth of the ramp,
// which takes twelve halvings to resolve. A full turn of the hue circle at high chroma still costs
// only a few hundred stops.
const MAX_DEPTH: u32 = 12;

/// The positions inside a segment where the approximation is checked, as fractions of it.
// Clipping into the sRGB gamut puts a corner in the curve, and an alpha near zero at one end puts a
// sharp bend there. Both tend to sit near an end, so the probes are spaced geometrically towards
// both ends, down to a 256th of the segment.
const PROBES: [f32; 17] = [
    1.0 / 256.0,
    1.0 / 128.0,
    1.0 / 64.0,
    1.0 / 32.0,
    1.0 / 16.0,
    1.0 / 8.0,
    1.0 / 4.0,
    3.0 / 8.0,
    1.0 / 2.0,
    5.0 / 8.0,
    3.0 / 4.0,
    7.0 / 8.0,
    15.0 / 16.0,
    31.0 / 32.0,
    63.0 / 64.0,
    127.0 / 128.0,
    255.0 / 256.0,
];

/// Returns sRGB stops that, interpolated linearly in premultiplied sRGB, approximate the ramp
/// `interpolation` describes through `stops`.
///
/// The approximation is held to [`DEFAULT_TOLERANCE`] per channel at the checked positions, and
/// each input segment is split into at most 4096 parts.
///
/// The offsets of `stops` must be non-decreasing. The result keeps every input stop, converted to
/// [`ColorSpace::Srgb`] with straight alpha, in the input order, and adds stops between them where
/// the ramp curves. Two stops at one offset stay a hard transition, and a ramp that is a straight
/// line in sRGB gains no stops.
///
/// ```
/// use iceblink::color::{Color, ColorSpace, GradientStop, Interpolation, densify};
///
/// let stops = [
///     GradientStop::new(0.0, Color::srgb(0.0, 0.0, 1.0, 1.0)),
///     GradientStop::new(1.0, Color::srgb(1.0, 1.0, 0.0, 1.0)),
/// ];
///
/// let oklab = densify(&stops, Interpolation::new(ColorSpace::Oklab));
/// assert!(oklab.len() > 2, "an Oklab ramp is not a straight line in sRGB");
/// assert_eq!(oklab[0].color.space(), ColorSpace::Srgb);
///
/// let srgb = densify(&stops, Interpolation::new(ColorSpace::Srgb));
/// assert_eq!(srgb.len(), 2, "an sRGB ramp is already what it approximates");
/// ```
pub fn densify(stops: &[GradientStop], interpolation: Interpolation) -> Vec<GradientStop> {
    densify_with_tolerance(stops, interpolation, DEFAULT_TOLERANCE)
}

/// Returns the stops of [`densify`] for a per-channel error bound of `tolerance`.
///
/// `tolerance` is in premultiplied sRGB channel units, where one is the whole range, so
/// `1.0 / 255.0` allows one step of an eight-bit channel. A smaller tolerance gives more stops,
/// and one at or below zero gives the most the subdivision depth allows.
pub fn densify_with_tolerance(
    stops: &[GradientStop],
    interpolation: Interpolation,
    tolerance: f32,
) -> Vec<GradientStop> {
    let to_srgb = |stop: &GradientStop| GradientStop::new(stop.offset, srgb(stop.color));
    if stops.len() < 2 {
        return stops.iter().map(to_srgb).collect();
    }

    let mut out = Vec::with_capacity(stops.len());
    out.push(to_srgb(&stops[0]));
    for pair in stops.windows(2) {
        let segment = Segment {
            from: pair[0],
            to: pair[1],
            interpolation,
            tolerance,
        };
        segment.subdivide(&mut out);
    }
    out
}

/// A pair of adjacent input stops and the ramp between them.
struct Segment {
    /// The stop the segment starts at.
    from: GradientStop,
    /// The stop the segment ends at.
    to: GradientStop,
    /// The space and hue arc the ramp is interpolated in.
    interpolation: Interpolation,
    /// The largest per-channel error the approximation may leave.
    tolerance: f32,
}

impl Segment {
    /// Appends the stops that approximate this segment, not including its first.
    fn subdivide(&self, out: &mut Vec<GradientStop>) {
        // The input colours are used as written, so a hard transition at one offset stays hard.
        let end = GradientStop::new(self.to.offset, srgb(self.to.color));
        let span = self.to.offset - self.from.offset;
        if span <= 0.0 || !span.is_finite() {
            out.push(end);
            return;
        }
        let start = GradientStop::new(self.from.offset, srgb(self.from.color));
        self.refine(0.0, start, 1.0, end, MAX_DEPTH, out);
    }

    /// Returns the stop `t` of the way along this segment, in sRGB.
    fn sample(&self, t: f32) -> GradientStop {
        let color = interpolate(self.from.color, self.to.color, t, self.interpolation);
        let offset = self.from.offset + (self.to.offset - self.from.offset) * t;
        GradientStop::new(offset, srgb(color))
    }

    /// Appends the stops approximating `start..=end`, not including `start`.
    fn refine(
        &self,
        start_t: f32,
        start: GradientStop,
        end_t: f32,
        end: GradientStop,
        depth: u32,
        out: &mut Vec<GradientStop>,
    ) {
        if depth > 0 && !self.is_close_enough(start_t, start, end_t, end) {
            let middle_t = f32::midpoint(start_t, end_t);
            let middle = self.sample(middle_t);
            self.refine(start_t, start, middle_t, middle, depth - 1, out);
            self.refine(middle_t, middle, end_t, end, depth - 1, out);
            return;
        }
        out.push(end);
    }

    /// Reports whether the straight line from `start` to `end` stays within the tolerance of the
    /// curve at every probe.
    fn is_close_enough(
        &self,
        start_t: f32,
        start: GradientStop,
        end_t: f32,
        end: GradientStop,
    ) -> bool {
        let start_color = start.color.to_premultiplied_srgb();
        let end_color = end.color.to_premultiplied_srgb();
        PROBES.into_iter().all(|probe| {
            let exact = self
                .sample(start_t + (end_t - start_t) * probe)
                .color
                .to_premultiplied_srgb();
            (0..4).all(|channel| {
                let chord =
                    start_color[channel] + (end_color[channel] - start_color[channel]) * probe;
                (exact[channel] - chord).abs() <= self.tolerance
            })
        })
    }
}

/// Converts a colour to sRGB.
fn srgb(color: Color) -> Color {
    color.to_space(ColorSpace::Srgb)
}
