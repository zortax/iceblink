//! Cutting a sprite's frame to the bounds of its item.

/// Cuts `frame` to `bounds` on `axis`, zero for x and one for y.
///
/// A corner whose radius on `axis` lies wholly outside the cut frame loses its rounding. A corner
/// cut by less than its radius keeps its radii, measured against the cut frame, so it draws inside
/// the uncut corner.
pub(crate) fn cut_frame(frame: &mut [f32; 4], radii: &mut [f32; 8], bounds: [f32; 4], axis: usize) {
    let start = frame[axis].max(bounds[axis]);
    let end = (frame[axis] + frame[2 + axis]).min(bounds[axis] + bounds[2 + axis]);
    let before = start - frame[axis];
    let after = frame[axis] + frame[2 + axis] - end;
    frame[axis] = start;
    frame[2 + axis] = (end - start).max(0.0);

    // The corners clockwise from the top left, by the side of the axis they sit on.
    let (low, high): ([usize; 2], [usize; 2]) = if axis == 0 {
        ([0, 3], [1, 2])
    } else {
        ([0, 1], [3, 2])
    };
    for (corners, cut) in [(low, before), (high, after)] {
        for corner in corners {
            if cut > 0.0 && cut >= radii[2 * corner + axis] {
                radii[2 * corner] = 0.0;
                radii[2 * corner + 1] = 0.0;
            }
        }
    }
}
