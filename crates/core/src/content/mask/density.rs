//! The transforms and sizes a path mask serves.

use iceblink::geom::Affine2;
use kurbo::Affine;

/// The most coverage texels one mask may hold.
const MAX_MASK_TEXELS: f32 = 64.0 * 1024.0;

/// The widest a mask may be.
const MAX_MASK_WIDTH: f32 = 1024.0;

/// The tallest a mask may be.
// A shelf allocator opens a shelf as tall as its tile, so a tall sliver spends a quarter of a
// page shared with every glyph, while a wide one costs almost nothing.
const MAX_MASK_HEIGHT: f32 = 256.0;

/// The largest off-axis coefficient, relative to the scale, of a linear part that still keeps the
/// axes.
const EPSILON: f32 = 1.0e-4;

/// Reports whether a mask of `width` by `height` texels may share a page with glyphs.
pub(super) fn fits(width: f32, height: f32) -> bool {
    width >= 1.0
        && height >= 1.0
        && width <= MAX_MASK_WIDTH
        && height <= MAX_MASK_HEIGHT
        && width * height <= MAX_MASK_TEXELS
}

/// Returns the mask texels per local unit along each axis of `affine`, or `None` when the
/// affine is degenerate, shears or turns by other than quarter turns.
///
/// A stroked path also needs both scales equal, since a stroke has one width.
pub(super) fn density_of(affine: Affine2, stroked: bool) -> Option<[f32; 2]> {
    const MIN_DENSITY: f32 = 1.0e-4;
    let kx = affine.a.hypot(affine.b);
    let ky = affine.c.hypot(affine.d);
    if !kx.is_finite() || !ky.is_finite() || kx < MIN_DENSITY || ky < MIN_DENSITY {
        return None;
    }

    let axial = affine.b.abs() <= EPSILON * kx && affine.c.abs() <= EPSILON * ky;
    let quarter = affine.a.abs() <= EPSILON * kx && affine.d.abs() <= EPSILON * ky;
    if !axial && !quarter {
        return None;
    }

    if stroked && (kx - ky).abs() > EPSILON * kx.max(ky) {
        return None;
    }
    Some([kx, ky])
}

/// Returns the sub-pixel shift that aligns mask texels with whole device pixels under `affine`.
///
/// The shift is zero unless `affine` is upright with positive scales.
pub(super) fn phase_of(affine: Affine2) -> [f32; 2] {
    if affine.b == 0.0 && affine.c == 0.0 && affine.a > 0.0 && affine.d > 0.0 {
        [affine.tx - affine.tx.floor(), affine.ty - affine.ty.floor()]
    } else {
        [0.0, 0.0]
    }
}

/// Reports whether `placement` scales both of its axes alike.
pub(super) fn uniform_axes(placement: Affine) -> bool {
    let [a, b, c, d, _, _] = placement.as_coeffs();
    let (kx, ky) = (a.hypot(b), c.hypot(d));
    (kx - ky).abs() <= f64::from(EPSILON) * kx.max(ky)
}
