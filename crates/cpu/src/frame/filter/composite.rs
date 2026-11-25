//! Compositing filtered content into the target beneath it.

use iceblink::BlendMode;
use iceblink::geom::{Device, Rect};

use crate::draw::Scene;
use crate::frame::filter::{EdgeMode, Filtered};
use crate::math::vec::{over, scale4};
use crate::target::Surface;

/// How filtered content is composited.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Composite {
    /// The device pixels written.
    pub(crate) bounds: Rect<i32, Device>,
    /// The frame clip slot applied, or `FRAME_NO_CLIP`.
    pub(crate) clip: u32,
    pub(crate) opacity: f32,
    pub(crate) blend: BlendMode,
}

/// Composites `filtered` into `surface` over the part of `how.bounds` inside `scissor`.
///
/// The shadows are drawn beneath the content, and the clip and the opacity scale both. The
/// result blends onto what `surface` holds with the composite's blend mode.
pub(crate) fn composite(
    scene: &Scene<'_>,
    surface: &mut Surface<'_>,
    scissor: Rect<i32, Device>,
    filtered: &Filtered,
    how: Composite,
    row: &mut Vec<[f32; 4]>,
) {
    let Some(area) = how
        .bounds
        .intersection(scissor)
        .and_then(|area| area.intersection(surface.extent()))
    else {
        return;
    };
    let clip = scene.clip(how.clip);
    let opacity = how.opacity.clamp(0.0, 1.0);
    let content = filtered.content.texture();
    let shadows: Vec<_> = filtered
        .shadows
        .iter()
        .map(|shadow| {
            (
                shadow.blurred.texture(),
                shadow.color,
                shadow.offset,
                shadow.edge,
            )
        })
        .collect();
    for y in area.top()..area.bottom() {
        row.clear();
        row.extend((area.left()..area.right()).map(|x| {
            let device = [x as f32 + 0.5, y as f32 + 0.5];
            let coverage = scene.clip_coverage(clip, device);
            if coverage <= 0.0 {
                return [0.0; 4];
            }
            let scale = coverage * opacity;
            let mut source = [0.0; 4];
            for (blurred, color, offset, edge) in &shadows {
                let moved = [device[0] - offset[0], device[1] - offset[1]];
                let alpha = match edge {
                    EdgeMode::Transparent => blurred.sample_transparent(moved)[3],
                    EdgeMode::Duplicate => blurred.sample(moved)[3],
                };
                source = over(source, scale4(*color, alpha * scale));
            }
            let sampled = filtered.matrix.apply(content.sample(device));
            over(source, scale4(sampled, scale))
        }));
        surface.blend_row(area.left(), y, row, how.blend);
    }
}
