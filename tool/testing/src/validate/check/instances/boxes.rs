//! Quads, shader quads, shadows and lines.

use iceblink::{FrameShadow, Line, Quad, ShaderQuad};

use crate::validate::Rule;
use crate::validate::check::Inspect;

pub(super) fn quad(inspect: &mut Inspect<'_>, position: usize, quad: &Quad) {
    let at = || format!("quads source {position}");
    inspect.clip(quad.clip.0, at);
    inspect.space(quad.space.0, at);
    inspect.paint(quad.fill, at);
    inspect.paint(quad.stroke, at);
    if !quad.style.is_valid() {
        inspect.push(
            Rule::Values,
            format!("{} has border style {}", at(), quad.style.0),
        );
    }
}

pub(super) fn shader_quad(inspect: &mut Inspect<'_>, position: usize, quad: &ShaderQuad) {
    let at = || format!("shader quads source {position}");
    inspect.clip(quad.clip.0, at);
    inspect.space(quad.space.0, at);
    inspect.paint(quad.fill, at);
    inspect.paint(quad.stroke, at);
    inspect.shader(quad.shader, at);
    inspect.params(quad.params, at);
}

pub(super) fn shadow(inspect: &mut Inspect<'_>, position: usize, shadow: &FrameShadow) {
    let at = || format!("shadows source {position}");
    inspect.clip(shadow.shadow.clip.0, at);
    inspect.space(shadow.shadow.space.0, at);
    if shadow.shadow.inset > 1 {
        inspect.push(
            Rule::Values,
            format!("{} has inset {}", at(), shadow.shadow.inset),
        );
    }
}

pub(super) fn line(inspect: &mut Inspect<'_>, position: usize, line: &Line) {
    let at = || format!("lines source {position}");
    inspect.clip(line.clip.0, at);
    inspect.space(line.space.0, at);
    if !line.style.is_valid() {
        inspect.push(
            Rule::Values,
            format!("{} has line style {}", at(), line.style.0),
        );
    }
}
