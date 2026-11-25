//! The rectangles a frame redraws, and the bands they are split into for drawing in parallel.

use iceblink::damage::DamageSet;
use iceblink::geom::{Device, Point, Rect};
use iceblink::{Frame, FrameBackdrop};

/// How many rows a band holds at least.
const BAND_ROWS: i32 = 32;

/// How many pixels a rectangle holds at least before it is split into bands.
const BANDED_AREA: i64 = 64 * 1024;

/// Returns the rectangles of `draw` within `used`.
///
/// A backdrop that reads past what it writes, and whose source the rectangles touch, has its
/// source within `used` added whole.
pub(crate) fn draw_rects(
    frame: &Frame,
    draw: &DamageSet,
    used: Rect<i32, Device>,
) -> Vec<Rect<i32, Device>> {
    let mut rects = if used.is_empty() {
        Vec::new()
    } else if draw.is_full() {
        vec![used]
    } else {
        draw.rects()
            .iter()
            .filter_map(|rect| rect.intersection(used))
            .collect()
    };
    if rects.is_empty() {
        return rects;
    }
    for source in reading(frame) {
        let Some(source) = source.intersection(used) else {
            continue;
        };
        // A backdrop the damage does not reach reads nothing this frame.
        if !rects.iter().any(|rect| rect.intersects(source)) {
            continue;
        }
        rects.retain(|rect| !source.contains_rect(*rect));
        if !rects.iter().any(|rect| rect.contains_rect(source)) {
            rects.push(source);
        }
    }
    rects
}

/// Returns the rows of `rect` split into bands of at least `BAND_ROWS` rows, up to three per
/// worker, none cutting through the rows a backdrop reads.
///
/// A rectangle smaller than `BANDED_AREA` is one band.
pub(crate) fn bands(
    frame: &Frame,
    rect: Rect<i32, Device>,
    workers: usize,
) -> Vec<Rect<i32, Device>> {
    let area = i64::from(rect.width()) * i64::from(rect.height());
    if workers <= 1 || area < BANDED_AREA {
        return vec![rect];
    }
    let reads: Vec<_> = reading(frame).collect();
    let count = (rect.height() / BAND_ROWS).clamp(1, workers as i32 * 3);
    let step = (rect.height() + count - 1) / count;
    let mut bands = Vec::new();
    let mut top = rect.top();
    while top < rect.bottom() {
        let mut bottom = (top + step).min(rect.bottom());
        // A cut inside a read moves down to the read's last row.
        while bottom < rect.bottom()
            && let Some(read) = reads
                .iter()
                .find(|read| read.top() < bottom && bottom < read.bottom())
        {
            bottom = read.bottom().min(rect.bottom());
        }
        bands.push(Rect::from_corners(
            Point::new(rect.left(), top),
            Point::new(rect.right(), bottom),
        ));
        top = bottom;
    }
    bands
}

// The rectangles backdrops read beyond what they write.
fn reading(frame: &Frame) -> impl Iterator<Item = Rect<i32, Device>> + '_ {
    frame
        .backdrops
        .iter()
        .filter(|backdrop| !reads_only_what_it_writes(backdrop))
        .map(|backdrop| rounded_out(backdrop.source))
}

/// Reports whether every filter of a backdrop reads only the pixel it writes.
pub(crate) fn reads_only_what_it_writes(backdrop: &FrameBackdrop) -> bool {
    backdrop
        .filters
        .iter()
        .all(|filter| filter.support() == [0.0; 4])
}

/// Returns the whole device pixels a rectangle `[x, y, width, height]` touches.
pub(crate) fn rounded_out(rect: [f32; 4]) -> Rect<i32, Device> {
    let [x, y, width, height] = rect;
    Rect::from_corners(
        Point::new(x.floor() as i32, y.floor() as i32),
        Point::new((x + width).ceil() as i32, (y + height).ceil() as i32),
    )
}
