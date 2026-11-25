//! Quads, shader quads, shadows and lines.

use iceblink::{BorderStyle, FrameShadow, Line, LineStyle, PaintKind, Quad, ROUND, ShaderQuad};

use crate::capture::mirror::Mirror;
use crate::capture::text::number::{all_zero, float, list, rect};
use crate::capture::transcript::{paint, space};

pub(crate) fn quad(mirror: &Mirror, quad: &Quad) -> String {
    let mut line = format!(
        "quad bounds={} fill={}",
        rect(quad.bounds),
        paint::reference(mirror, quad.fill)
    );
    if !all_zero(&quad.border) {
        line.push_str(&format!(
            " border={} stroke={} style={}",
            list(&quad.border),
            paint::reference(mirror, quad.stroke),
            border(quad.style)
        ));
    }
    corners(&mut line, &quad.radii, &quad.shapes);
    origin(&mut line, quad.paint_origin);
    line.push_str(&space::suffix(mirror, quad.clip.0, quad.space.0));
    line
}

pub(crate) fn shader_quad(mirror: &Mirror, quad: &ShaderQuad) -> String {
    let mut line = format!(
        "shader_quad bounds={} shader=#{} params=#{}",
        rect(quad.bounds),
        quad.shader,
        quad.params
    );
    if quad.fill.kind != PaintKind::NONE {
        line.push_str(&format!(" fill={}", paint::reference(mirror, quad.fill)));
    }
    if !all_zero(&quad.border) {
        line.push_str(&format!(
            " border={} stroke={}",
            list(&quad.border),
            paint::reference(mirror, quad.stroke)
        ));
    }
    corners(&mut line, &quad.radii, &quad.shapes);
    origin(&mut line, quad.paint_origin);
    if quad.opacity != 1.0 {
        line.push_str(&format!(" opacity={}", float(quad.opacity)));
    }
    line.push_str(&space::suffix(mirror, quad.clip.0, quad.space.0));
    line
}

pub(crate) fn shadow(mirror: &Mirror, framed: &FrameShadow) -> String {
    let shadow = &framed.shadow;
    let mut line = format!(
        "shadow bounds={} shape={} blur={} color={} caster={}",
        rect(framed.bounds),
        rect(shadow.shape_bounds),
        float(shadow.blur),
        paint::color(shadow.color),
        rect(shadow.caster_bounds)
    );
    if !all_zero(&shadow.radii) {
        line.push_str(&format!(" radii={}", list(&shadow.radii)));
    }
    if !all_zero(&shadow.caster_radii) {
        line.push_str(&format!(" caster_radii={}", list(&shadow.caster_radii)));
    }
    if shadow.shapes != ROUND {
        line.push_str(&format!(" shapes={}", list(&shadow.shapes)));
    }
    if shadow.inset != 0 {
        line.push_str(" inset");
    }
    line.push_str(&space::suffix(mirror, shadow.clip.0, shadow.space.0));
    line
}

pub(crate) fn line(mirror: &Mirror, decoration: &Line) -> String {
    let mut line = format!(
        "line bounds={} color={} thickness={} style={}",
        rect(decoration.bounds),
        paint::color(decoration.color),
        float(decoration.thickness),
        line_style(decoration.style)
    );
    line.push_str(&space::suffix(
        mirror,
        decoration.clip.0,
        decoration.space.0,
    ));
    line
}

/// Appends the corner radii unless they are all zero, and the corner shapes unless they are round.
pub(super) fn corners(line: &mut String, radii: &[f32; 8], shapes: &[f32; 4]) {
    if !all_zero(radii) {
        line.push_str(&format!(" radii={}", list(radii)));
    }
    if *shapes != ROUND {
        line.push_str(&format!(" shapes={}", list(shapes)));
    }
}

/// Appends the paint origin unless it is zero.
pub(super) fn origin(line: &mut String, paint_origin: [f32; 2]) {
    if !all_zero(&paint_origin) {
        line.push_str(&format!(" paint_origin={}", list(&paint_origin)));
    }
}

fn border(style: BorderStyle) -> String {
    match style {
        BorderStyle::SOLID => "solid".to_owned(),
        BorderStyle::DASHED => "dashed".to_owned(),
        BorderStyle::DOTTED => "dotted".to_owned(),
        other => format!("<style {}>", other.0),
    }
}

fn line_style(style: LineStyle) -> String {
    match style {
        LineStyle::SOLID => "solid".to_owned(),
        LineStyle::WAVY => "wavy".to_owned(),
        LineStyle::DASHED => "dashed".to_owned(),
        LineStyle::DOTTED => "dotted".to_owned(),
        LineStyle::DOUBLE => "double".to_owned(),
        other => format!("<style {}>", other.0),
    }
}
