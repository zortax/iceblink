//! Outlines cut into horizontal bands of whole pixels.

use crate::vector_coverage::geometry::Segment;

/// The most bands one outline has.
// A band is one pixel tall until an outline is taller than this, and taller bands after that.
const MAX_BANDS: usize = 4096;

/// Where the bands of one outline are, and the rows they cover.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Banding {
    /// The first band in the band table.
    pub(super) first: u32,
    /// How many bands there are, zero for an outline with nothing to fill.
    pub(super) count: u32,
    /// The pixel row the first band starts at.
    pub(super) top: f32,
    /// The height of one band, a whole number of pixels and at least one.
    pub(super) tall: f32,
}

/// Appends the bands of `outline` over `bounds` to the band table `bands`, and the segments of
/// each band to `segments`, using `tally` as scratch.
///
/// `bounds` is origin and extent. Bands start on a pixel row and are whole pixels tall, so every
/// pixel lies in one band, and each band holds a copy of every segment whose rows reach into it.
// A fragment then walks the few segments of its band, once for all sixteen samples, instead of
// every segment of the outline.
pub(super) fn band(
    outline: &[Segment],
    bounds: [f32; 4],
    bands: &mut Vec<[u32; 4]>,
    segments: &mut Vec<Segment>,
    tally: &mut Vec<u32>,
) -> Banding {
    let top = bounds[1].floor();
    let first = bands.len() as u32;
    let height = bounds[1] + bounds[3] - top;
    // A NaN height would otherwise give no bands and test the outline against nothing.
    if outline.is_empty() || !height.is_finite() || height <= 0.0 {
        return Banding {
            first,
            count: 0,
            top,
            tall: 1.0,
        };
    }
    let tall = (height / MAX_BANDS as f32).ceil().max(1.0);
    let count = ((height / tall).ceil() as usize).clamp(1, MAX_BANDS);

    // Counted first and placed second, so each band's segments lie together without a list per
    // band. `tally` holds each band's count, and then where each band starts.
    tally.clear();
    tally.resize(count, 0);
    for &segment in outline {
        let (low, high) = span(segment, top, tall, count);
        for slot in &mut tally[low..=high] {
            *slot += 1;
        }
    }
    let base = segments.len();
    let mut running = 0;
    for slot in tally.iter_mut() {
        let held = *slot;
        *slot = running;
        bands.push([base as u32 + running, held, 0, 0]);
        running += held;
    }
    segments.resize(base + running as usize, [0.0; 4]);
    for &segment in outline {
        let (low, high) = span(segment, top, tall, count);
        for slot in &mut tally[low..=high] {
            segments[base + *slot as usize] = segment;
            *slot += 1;
        }
    }

    Banding {
        first,
        count: count as u32,
        top,
        tall,
    }
}

/// Returns the first and last band, both inclusive, that the rows of `segment` reach.
///
/// A segment outside the bands is clamped into the nearest one, where it crosses no sample row.
fn span(segment: Segment, top: f32, tall: f32, count: usize) -> (usize, usize) {
    let last = count.saturating_sub(1) as isize;
    let of = |y: f32| (((y - top) / tall).floor() as isize).clamp(0, last) as usize;
    (
        of(segment[1].min(segment[3])),
        of(segment[1].max(segment[3])),
    )
}
