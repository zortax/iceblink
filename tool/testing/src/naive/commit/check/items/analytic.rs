//! Quads, shader quads, shadows and lines.

use iceblink::{Line, Quad, RejectReason, ShaderQuad, Shadow};

use crate::naive::commit::check::finite::{color, numbers};
use crate::naive::commit::check::items::scope::Scope;
use crate::naive::commit::check::nodes::valid;

pub(super) fn quad(scope: &Scope<'_>, quad: &Quad) -> Result<(), RejectReason> {
    numbers(&quad.bounds)?;
    numbers(&quad.radii)?;
    numbers(&quad.shapes)?;
    numbers(&quad.border)?;
    numbers(&quad.paint_origin)?;
    valid(quad.style.is_valid())?;
    scope.paint(quad.fill)?;
    scope.paint(quad.stroke)?;
    scope.clip(quad.clip)?;
    scope.space(quad.space)
}

pub(super) fn shader_quad(scope: &Scope<'_>, quad: &ShaderQuad) -> Result<(), RejectReason> {
    numbers(&quad.bounds)?;
    numbers(&quad.radii)?;
    numbers(&quad.shapes)?;
    numbers(&quad.border)?;
    numbers(&quad.paint_origin)?;
    numbers(&[quad.opacity])?;
    scope.paint(quad.fill)?;
    scope.paint(quad.stroke)?;
    scope.clip(quad.clip)?;
    scope.space(quad.space)?;
    scope.params(quad.params)?;
    scope.quad_shader(quad.shader)
}

pub(super) fn shadow(scope: &Scope<'_>, shadow: &Shadow) -> Result<(), RejectReason> {
    numbers(&shadow.shape_bounds)?;
    numbers(&shadow.radii)?;
    numbers(&shadow.caster_bounds)?;
    numbers(&shadow.caster_radii)?;
    numbers(&shadow.shapes)?;
    numbers(&[shadow.blur])?;
    color(shadow.color)?;
    valid(shadow.inset <= 1)?;
    scope.clip(shadow.clip)?;
    scope.space(shadow.space)
}

pub(super) fn line(scope: &Scope<'_>, line: &Line) -> Result<(), RejectReason> {
    numbers(&line.bounds)?;
    numbers(&[line.thickness])?;
    color(line.color)?;
    valid(line.style.is_valid())?;
    scope.clip(line.clip)?;
    scope.space(line.space)
}
