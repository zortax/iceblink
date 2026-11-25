//! Paint references, resolved through the paint table.

use iceblink::{Color, FramePaint, FrameStop, GradientShape, PaintKind, PaintRef, StopEncoding};

use crate::capture::mirror::Mirror;
use crate::capture::text::number::{float, list};

/// Renders a premultiplied colour.
pub(crate) fn color(color: Color) -> String {
    format!("premul{}", list(&color.0))
}

/// Renders the paint `paint` refers to.
///
/// A reference whose kind differs from its paint renders as the mismatch, and a slot the mirror
/// lacks renders as missing.
pub(crate) fn reference(mirror: &Mirror, paint: PaintRef) -> String {
    if paint.kind == PaintKind::NONE {
        return "none".to_owned();
    }

    let declared = kind(paint.kind);
    let Some(held) = mirror.paints.get(paint.index as usize) else {
        return format!("{declared}#{} <missing>", paint.index);
    };
    let actual = match held {
        FramePaint::None => PaintKind::NONE,
        FramePaint::Solid(_) => PaintKind::SOLID,
        FramePaint::Gradient(_) => PaintKind::GRADIENT,
        FramePaint::Sides(_) => PaintKind::SIDES,
    };
    if actual != paint.kind {
        return format!("<mismatched> declared={declared} entry={}", value(held));
    }
    value(held)
}

/// Renders a paint.
pub(crate) fn value(paint: &FramePaint) -> String {
    match paint {
        FramePaint::None => "none".to_owned(),
        FramePaint::Solid(fill) => format!("solid {}", color(*fill)),
        FramePaint::Sides(sides) => {
            let rendered: Vec<String> = sides.iter().copied().map(color).collect();
            format!("sides [{}]", rendered.join(", "))
        }
        FramePaint::Gradient(gradient) => {
            let stops: Vec<String> = gradient.stops.iter().map(stop).collect();
            format!(
                "{} in {} stops=[{}]{}",
                shape(&gradient.shape),
                encoding(gradient.encoding),
                stops.join(", "),
                if gradient.repeat { " repeat" } else { "" }
            )
        }
    }
}

fn stop(stop: &FrameStop) -> String {
    format!("{}@{}", list(&stop.color), float(stop.offset))
}

fn shape(shape: &GradientShape) -> String {
    match *shape {
        GradientShape::Linear { start, end } => format!(
            "linear from=({}, {}) to=({}, {})",
            float(start[0]),
            float(start[1]),
            float(end[0]),
            float(end[1])
        ),
        GradientShape::Radial { center, radius } => format!(
            "radial at=({}, {}) radii=({}, {})",
            float(center[0]),
            float(center[1]),
            float(radius[0]),
            float(radius[1])
        ),
        GradientShape::Conic { center, angle } => format!(
            "conic at=({}, {}) from={}rad",
            float(center[0]),
            float(center[1]),
            float(angle)
        ),
    }
}

fn encoding(encoding: StopEncoding) -> String {
    match encoding {
        StopEncoding::SRGB => "srgb".to_owned(),
        StopEncoding::OKLAB => "oklab".to_owned(),
        StopEncoding::LINEAR_SRGB => "linear_srgb".to_owned(),
        other => format!("<encoding {}>", other.0),
    }
}

fn kind(kind: PaintKind) -> String {
    match kind {
        PaintKind::NONE => "none".to_owned(),
        PaintKind::SOLID => "solid".to_owned(),
        PaintKind::GRADIENT => "gradient".to_owned(),
        PaintKind::SIDES => "sides".to_owned(),
        PaintKind::INLINE => "inline".to_owned(),
        other => format!("<kind {}>", other.0),
    }
}
