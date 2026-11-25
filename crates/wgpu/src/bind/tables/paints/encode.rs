//! Encoding frame paints into paint records and their stops.

use iceblink::{FramePaint, GradientShape};

use crate::bind::tables::records::{GpuPaint, GpuStop};

/// The paint kinds of [`GpuPaint`], which match `PaintKind`.
pub(crate) mod kind {
    pub(crate) const SOLID: u32 = 1;
    pub(crate) const GRADIENT: u32 = 2;
    pub(crate) const SIDES: u32 = 4;
}

/// Returns the record of `paint` without its stop range, and writes its stops into `stops`.
pub(crate) fn encode(paint: &FramePaint, stops: &mut Vec<GpuStop>) -> GpuPaint {
    stops.clear();
    match paint {
        FramePaint::None => GpuPaint::default(),
        FramePaint::Solid(color) => GpuPaint {
            kind: kind::SOLID,
            color: color.0,
            ..GpuPaint::default()
        },
        FramePaint::Sides(colors) => {
            stops.extend(colors.iter().map(|color| GpuStop {
                color: color.0,
                ..GpuStop::default()
            }));
            GpuPaint {
                kind: kind::SIDES,
                ..GpuPaint::default()
            }
        }
        FramePaint::Gradient(gradient) => {
            stops.extend(gradient.stops.iter().map(|stop| GpuStop {
                color: stop.color,
                offset: stop.offset,
                pad: [0.0; 3],
            }));
            let (tag, geometry) = match gradient.shape {
                GradientShape::Linear { start, end } => (0, [start[0], start[1], end[0], end[1]]),
                GradientShape::Radial { center, radius } => {
                    (1, [center[0], center[1], radius[0], radius[1]])
                }
                GradientShape::Conic { center, angle } => (2, [center[0], center[1], 0.0, angle]),
            };
            GpuPaint {
                kind: kind::GRADIENT,
                gradient: tag,
                space: u32::from(gradient.encoding.0),
                flags: u32::from(gradient.repeat),
                geometry,
                ..GpuPaint::default()
            }
        }
    }
}
