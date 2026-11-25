//! Opaque covers: quads that let a renderer skip what lies beneath them.

use iceblink::geom::{Device, DevicePx, Point, Rect, Size};
use iceblink::{
    BorderStyle, DeviceRect, FRAME_NO_CLIP, FrameClipShape, FramePaint, Matrix4, PaintKind,
    PaintRef, Quad,
};

use crate::geometry::bounds::map;
use crate::plan::inner_pixels;
use crate::tables::FrameTables;

/// Returns the pixels a quad makes opaque, or `None` when the quad is not an opaque cover.
///
/// The pixels are computed as [`crate::plan::Emission::push_cover`] describes, and cut to
/// `visible`, the quad's ink on the output.
pub(super) fn cover(tables: &FrameTables, quad: &Quad, visible: DeviceRect) -> Option<DeviceRect> {
    let bordered = quad.border.iter().any(|&width| width != 0.0);
    let solid_border = quad.style == BorderStyle::SOLID && opaque(tables, quad.stroke);
    if !opaque(tables, quad.fill) || (bordered && !solid_border) {
        return None;
    }

    let world = tables.spaces.get(quad.space.0)?;
    let scale = upright(world)?;
    let mut bounds = map(world, quad.bounds);

    if quad.clip.0 != FRAME_NO_CLIP {
        for link in &tables.clips.get(quad.clip.0)?.links {
            let FrameClipShape::RoundedRect { rect, radii, .. } = &link.shape else {
                return None;
            };
            if radii.iter().any(|&radius| radius != 0.0) {
                return None;
            }
            let world = tables.spaces.get(link.space)?;
            upright(world)?;
            bounds = bounds.intersect(map(world, *rect));
        }
    }

    let radius = quad
        .radii
        .iter()
        .fold(0.0_f32, |held, &radius| held.max(radius))
        * scale;
    let inset = bounds.outset([-radius; 4]);
    if inset.is_empty() {
        return None;
    }

    let rect: Rect<DevicePx, Device> = Rect::new(
        Point::new(DevicePx(inset.x0), DevicePx(inset.y0)),
        Size::new(DevicePx(inset.x1 - inset.x0), DevicePx(inset.y1 - inset.y0)),
    );
    let pixels = inner_pixels(rect).intersection(visible)?;
    (pixels.size.width > 0 && pixels.size.height > 0).then_some(pixels)
}

/// Reports whether a paint is an opaque solid colour.
fn opaque(tables: &FrameTables, paint: PaintRef) -> bool {
    paint.kind == PaintKind::SOLID
        && matches!(
            tables.paints.get(paint.index),
            Some(FramePaint::Solid(color)) if color.is_opaque()
        )
}

/// Returns the larger scale factor of a translation or positive scale.
fn upright(world: &Matrix4) -> Option<f32> {
    let affine = world.to_affine2()?;
    (affine.b == 0.0 && affine.c == 0.0 && affine.a > 0.0 && affine.d > 0.0)
        .then_some(affine.a.max(affine.d))
}
